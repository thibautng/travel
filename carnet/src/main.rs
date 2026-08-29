//! `carnet`, pipeline médias du site des voyages.
//!
//! Voir SPEC.md, section 6. Les lots 1 et 2 implémentent `scan`, `stats`,
//! `check` et la part de `build` qui ne produit pas de dérivés.

use anyhow::{bail, Context, Result};
use carnet::{emit, itineraire, jours, lieux, overrides, quality, scan, track, voyage};
use clap::{Parser, Subcommand};
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
    /// Inventaire, surcharges, traces, et dérivés au lot 3
    Build { voyage: String },
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

        Commande::Build { voyage } => {
            let p = executer(&depot, &voyage, false)?;
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
            println!("Les dérivés d'images arrivent au lot 3, les vidéos attendent la sélection (D8).");
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
