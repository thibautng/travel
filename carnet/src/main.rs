//! `carnet`, pipeline médias du site des voyages.
//!
//! Voir SPEC.md, section 6. Le lot 1 implémente `scan` et `stats`.

use anyhow::{bail, Context, Result};
use carnet::{emit, quality, scan, voyage};
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
    /// Inventaire, surcharges, dérivés et traces
    Build {
        voyage: String,
    },
    /// Contrôles de cohérence, sans rien écrire
    Check {
        voyage: String,
    },
    /// Récapitulatif : médias, couverture GPS, anomalies
    Stats {
        voyage: String,
    },
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
            let v = voyage::Voyage::charger(&depot, &voyage)?;
            let mut inventaire = scan::inventorier(&v, false)?;
            let bilan = quality::evaluer(&mut inventaire.medias, &v);
            emit::rapport(&v, &inventaire, &bilan);
            Ok(())
        }
        Commande::Build { .. } => {
            bail!("carnet build arrive au lot 3. Voir SPEC.md, section 11.")
        }
        Commande::Check { .. } => {
            bail!("carnet check arrive au lot 2. Voir SPEC.md, section 11.")
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
