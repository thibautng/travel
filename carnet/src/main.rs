//! `carnet`, pipeline médias du site des voyages.
//!
//! Voir SPEC.md, section 6. Les lots 1 et 2 implémentent `scan`, `stats`,
//! `check` et la part de `build` qui ne produit pas de dérivés.

use anyhow::{bail, Context, Result};
use carnet::{derive, emit, itineraire, jours, lieux, overrides, quality, scan, selection, track, voyage};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "carnet",
    version,
    about = "Pipeline médias du site des voyages",
    long_about = "Produit data/ et media/ à partir des dossiers photo, en suivant SPEC.md."
)]
struct Cli {
    /// Racine du dépôt. Par défaut, cherchée en remontant depuis le dossier courant.
    #[arg(long, global = true, value_name = "CHEMIN")]
    depot: Option<PathBuf>,

    #[command(subcommand)]
    commande: Commande,
}

#[derive(Subcommand)]
enum Commande {
    /// Lit le dossier source et produit un inventaire brut
    Scan {
        /// Identifiant du voyage, par exemple 2026-alpes
        voyage: String,
    },
    /// Inventaire, surcharges, traces et dérivés
    Build {
        voyage: String,
        /// Format des dérivés : `avif` ou `jpeg`.
        ///
        /// L'AVIF est la cible. Le JPEG existe pour les machines sans AVX2,
        /// où l'encodeur AVIF perd un ordre de grandeur.
        #[arg(long, default_value = "avif", value_name = "FORMAT")]
        format: String,
        /// Recalculer les dérivés même si le cache les dit à jour.
        #[arg(long)]
        force: bool,
        /// Ne produire aucun dérivé : seules les données de `data/` sont écrites.
        #[arg(long)]
        sans_derives: bool,
    },
    /// Contrôles de cohérence, sans rien écrire
    Check { voyage: String },
    /// Récapitulatif : médias, couverture GPS, anomalies
    Stats { voyage: String },
}

/// Remonte depuis `depart` jusqu'à trouver la racine du dépôt.
fn trouver_depot(depart: &Path) -> Result<PathBuf> {
    let mut courant = Some(depart);
    while let Some(dossier) = courant {
        if dossier.join("content").join("voyages").is_dir() {
            return Ok(dossier.to_path_buf());
        }
        courant = dossier.parent();
    }
    bail!(
        "racine du dépôt introuvable depuis {} : aucun dossier parent ne contient content/voyages. \
         Préciser --depot.",
        depart.display()
    )
}

/// Tout ce que le pipeline a produit en mémoire, avant écriture.
struct Pipeline {
    voyage: voyage::Voyage,
    inventaire: scan::Inventaire,
    journees: Vec<jours::Journee>,
    journal: overrides::Journal,
    bilan_lieux: lieux::Bilan,
    bilan_qualite: quality::Bilan,
    traces: track::Traces,
    itineraires: itineraire::Itineraires,
}

/// Enchaîne les étapes de la section 6.2, jusqu'aux traces.
///
/// Les dérivés, étapes 8 à 11, arrivent au lot 3. `build` et `check`
/// partagent ce chemin : ce qui les distingue est ce qu'ils en font.
fn executer(depot: &Path, voyage_id: &str, silencieux: bool) -> Result<Pipeline> {
    let voyage = voyage::Voyage::charger(depot, voyage_id)?;

    // Étapes 1 à 3 : inventaire, datation, scoring.
    let mut inventaire = scan::inventorier(&voyage, silencieux)?;
    let mut bilan_qualite = quality::evaluer(&mut inventaire.medias, &voyage);

    // Étape 4 : surcharges.
    let surcharges = overrides::Overrides::charger(depot, voyage_id)?;
    let journal = surcharges.appliquer(&mut inventaire.medias, voyage.fuseau);

    // Étapes 6 et 7 : héritage et interpolation.
    let journees = jours::charger(depot, voyage_id)?;
    let bilan_lieux = lieux::appliquer(&mut inventaire.medias, &voyage, &journees);

    // Les trous candidats sont recalculés après corrections : signaler un
    // trou déjà comblé par overrides.yaml ferait du bruit pour rien.
    bilan_qualite.trous = quality::trous_candidats(&inventaire.medias);

    // Étapes 12 et 13 : itinéraires et traces.
    let mut itineraires = itineraire::Itineraires::charger(depot, voyage_id)?;
    let traces = track::construire(
        &inventaire.medias,
        &voyage,
        &journees,
        &surcharges,
        &mut itineraires,
    );

    Ok(Pipeline {
        voyage,
        inventaire,
        journees,
        journal,
        bilan_lieux,
        bilan_qualite,
        traces,
        itineraires,
    })
}

struct BilanDerives {
    produits: usize,
    caches: usize,
    echecs: Vec<String>,
    secondes: f64,
    octets: u64,
}

/// Go et Mo au sens décimal, comme SPEC.md, section 8, C7.
fn octets_lisibles(octets: u64) -> String {
    let go = octets as f64 / 1_000_000_000.0;
    if go >= 1.0 {
        return format!("{go:.1} Go").replace('.', ",");
    }
    format!("{:.0} Mo", octets as f64 / 1_000_000.0)
}

/// Étapes 9 et 10 : dérivés et aperçus, pour les seuls médias retenus.
///
/// Les vidéos ne sont pas touchées : leur transcodage attend la sélection
/// (D8). Un échec sur une image ne fait pas échouer le build, il est compté
/// et rapporté : mieux vaut 704 dérivés et une ligne d'erreur que rien.
fn deriver(
    depot: &Path,
    p: &mut Pipeline,
    reglages: &derive::Reglages,
    force: bool,
) -> Result<BilanDerives> {
    use rayon::prelude::*;

    let dossier = depot.join("media").join(&p.voyage.id);
    let mut cache = derive::CacheBuild::charger(depot, &p.voyage.id, reglages);

    let mut a_produire: Vec<(usize, scan::Media, PathBuf, (u64, i64))> = Vec::new();
    let mut caches = 0usize;

    for indice in 0..p.inventaire.medias.len() {
        let media = &p.inventaire.medias[indice];
        if media.type_media != scan::TypeMedia::Photo || !media.publie {
            continue;
        }
        let source = p.voyage.source_photos.join(&media.fichier_source);
        let Some(signature) = derive::signature(&source) else {
            continue;
        };

        if !force {
            if let Some(entree) = cache.valide(&media.id, signature, &dossier) {
                let (derives, lqip) = (entree.derives.clone(), entree.lqip.clone());
                let (largeur, hauteur) = (entree.largeur, entree.hauteur);
                let media = &mut p.inventaire.medias[indice];
                media.derives = Some(derives);
                media.lqip = Some(lqip);
                media.largeur = Some(largeur);
                media.hauteur = Some(hauteur);
                caches += 1;
                continue;
            }
        }
        a_produire.push((indice, media.clone(), source, signature));
    }

    let barre = ProgressBar::new(a_produire.len() as u64);
    barre.set_style(
        ProgressStyle::with_template("  {bar:40} {pos}/{len} dérivés  {eta}")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    let debut = std::time::Instant::now();
    type Resultat = (
        usize,
        (u64, i64),
        Result<derive::Production, derive::ErreurDerive>,
    );
    let resultats: Vec<Resultat> =
        a_produire
            .par_iter()
            .map(|(indice, media, source, signature)| {
                let resultat = derive::produire(media, source, &dossier, reglages);
                barre.inc(1);
                (*indice, *signature, resultat)
            })
            .collect();
    barre.finish_and_clear();
    let secondes = debut.elapsed().as_secs_f64();

    let mut bilan = BilanDerives {
        produits: 0,
        caches,
        echecs: Vec::new(),
        secondes,
        octets: 0,
    };

    for (indice, signature, resultat) in resultats {
        match resultat {
            Ok(production) => {
                cache.inserer(
                    &p.inventaire.medias[indice].id,
                    derive::EntreeCache {
                        octets: signature.0,
                        mtime: signature.1,
                        derives: production.derives.clone(),
                        lqip: production.lqip.clone(),
                        largeur: production.largeur,
                        hauteur: production.hauteur,
                    },
                );
                let media = &mut p.inventaire.medias[indice];
                media.derives = Some(production.derives);
                media.lqip = Some(production.lqip);
                media.largeur = Some(production.largeur);
                media.hauteur = Some(production.hauteur);
                bilan.produits += 1;
            }
            Err(erreur) => bilan.echecs.push(format!(
                "{} : {erreur}",
                p.inventaire.medias[indice].id
            )),
        }
    }

    cache.enregistrer()?;

    if dossier.is_dir() {
        bilan.octets = walkdir::WalkDir::new(&dossier)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok().map(|m| m.len()))
            .sum();
    }
    Ok(bilan)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let depot = match cli.depot {
        Some(chemin) => chemin,
        None => {
            let courant = std::env::current_dir().context("dossier courant illisible")?;
            trouver_depot(&courant)?
        }
    };

    match cli.commande {
        Commande::Scan { voyage } => {
            let v = voyage::Voyage::charger(&depot, &voyage)?;
            println!("Inventaire de « {} »", v.titre);
            let mut inventaire = scan::inventorier(&v, false)?;
            let bilan = quality::evaluer(&mut inventaire.medias, &v);
            let chemin = emit::ecrire_media_json(&depot, &v, &inventaire.medias)?;
            println!(
                "  {} médias inventoriés, {} trous de trace candidats",
                inventaire.medias.len(),
                bilan.trous.len()
            );
            println!("  écrit : {}", chemin.display());
            println!("  détail : carnet stats {}", v.id);
            Ok(())
        }

        Commande::Stats { voyage } => {
            let p = executer(&depot, &voyage, false)?;
            emit::rapport(&p.voyage, &p.inventaire, &p.bilan_qualite);
            emit::rapport_traces(&p.traces, &p.journal, &p.bilan_lieux, &p.itineraires);
            Ok(())
        }

        Commande::Check { voyage } => {
            let p = executer(&depot, &voyage, true)?;
            emit::rapport_traces(&p.traces, &p.journal, &p.bilan_lieux, &p.itineraires);

            // Aucun arbitrage silencieux : une surcharge qui ne vise rien, ou
            // une référence de lieu inconnue, font échouer la commande.
            let mut fautes: Vec<String> = p.journal.inutilisees.clone();
            let connus: Vec<&str> = p.voyage.lieux.iter().map(|l| l.id.as_str()).collect();
            for journee in &p.journees {
                for (champ, valeur) in [("lieu", &journee.lieu), ("camp", &journee.camp)] {
                    if let Some(id) = valeur {
                        if !connus.contains(&id.as_str()) {
                            fautes.push(format!(
                                "journée {} : {champ} « {id} » absent de voyage.yaml",
                                journee.date
                            ));
                        }
                    }
                }
            }
            for jour in &p.traces.bilan.repartitions_sans_segment {
                fautes.push(format!(
                    "répartition demandée le {jour}, mais aucun segment manuel ce jour-là"
                ));
            }

            if fautes.is_empty() {
                println!("carnet check : rien à signaler.");
                return Ok(());
            }
            for faute in &fautes {
                eprintln!("  {faute}");
            }
            bail!("{} incohérence(s), voir ci-dessus", fautes.len())
        }

        Commande::Build {
            voyage,
            format,
            force,
            sans_derives,
        } => {
            let mut p = executer(&depot, &voyage, false)?;

            // Étape 8 : sélection. Voir D7.
            let choix = selection::Selection::charger(&depot, &voyage)?;
            let cites = selection::citations_du_recit(&depot, &voyage)?;
            let bilan_selection =
                selection::appliquer(&mut p.inventaire.medias, &choix, &cites);

            // Étapes 9 et 10 : dérivés et aperçus.
            let reglages = derive::Reglages {
                format: derive::Format::depuis_nom(&format).ok_or_else(|| {
                    anyhow::anyhow!("format « {format} » inconnu : avif ou jpeg")
                })?,
                ..derive::Reglages::default()
            };
            let bilan_derives = if sans_derives {
                None
            } else {
                Some(deriver(&depot, &mut p, &reglages, force)?)
            };
            let media_json = emit::ecrire_media_json(&depot, &p.voyage, &p.inventaire.medias)?;
            let jours_agreges = emit::construire_jours(
                &p.inventaire.medias,
                &p.journees,
                &p.traces,
                &p.bilan_qualite,
            );
            let jours_json = emit::ecrire_jours_json(&depot, &p.voyage, &jours_agreges)?;
            let trace = emit::ecrire_trace_geojson(&depot, &p.voyage, &p.traces)?;
            let cache = p.itineraires.enregistrer()?;

            emit::rapport_traces(&p.traces, &p.journal, &p.bilan_lieux, &p.itineraires);

            println!();
            println!("SÉLECTION (D7)");
            if bilan_selection.tout_retenu {
                println!("  aucune restriction déclarée : les {} médias sont publiés", bilan_selection.publies);
            } else {
                println!(
                    "  {} publiés, {} écartés, dont {} cités par le récit",
                    bilan_selection.publies, bilan_selection.ecartes, bilan_selection.cites_par_le_recit
                );
            }
            for faute in &bilan_selection.inutilisees {
                println!("  INUTILISÉE   {faute}");
            }

            if let Some(bilan) = &bilan_derives {
                println!();
                println!("DÉRIVÉS (étapes 9 et 10)");
                println!("  format        {}", format);
                println!("  produits      {}", bilan.produits);
                println!("  repris du cache {}", bilan.caches);
                println!("  en échec      {}", bilan.echecs.len());
                for echec in bilan.echecs.iter().take(5) {
                    println!("    {echec}");
                }
                println!(
                    "  temps         {:.1} s pour {} photos",
                    bilan.secondes, bilan.produits
                );
                println!("  poids         {}", octets_lisibles(bilan.octets));
            }

            println!();
            println!("ÉCRIT");
            for chemin in [Some(media_json), Some(jours_json), Some(trace)]
                .into_iter()
                .flatten()
            {
                println!("  {}", chemin.display());
            }
            if let Some(chemin) = cache {
                println!("  {}", chemin.display());
            }
            println!();
            println!("Le transcodage vidéo attend la sélection des vidéos (D8).");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depot_introuvable_donne_une_erreur_explicite() {
        let erreur = trouver_depot(Path::new("/dossier/qui/n/existe/pas")).unwrap_err();
        assert!(erreur.to_string().contains("content/voyages"));
    }
}
