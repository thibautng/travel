//! Lecture du frontmatter des journées, `content/voyages/<id>/jours/*.md`.
//!
//! Voir SPEC.md, section 5.2. Le pipeline ne lit que les métadonnées : le
//! corps du récit ne le concerne pas, et `data/` ne contient jamais de
//! contenu rédactionnel (section 4).

use chrono::NaiveDate;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ErreurJours {
    #[error("lecture de {chemin} impossible")]
    Lecture {
        chemin: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{chemin} ne commence pas par un frontmatter délimité par ---")]
    FrontmatterAbsent { chemin: PathBuf },

    #[error("le frontmatter de {chemin} est mal formé")]
    Syntaxe {
        chemin: PathBuf,
        #[source]
        source: serde_norway::Error,
    },

    #[error("{chemin} déclare la date {declaree}, son nom annonce {attendue}")]
    DateIncoherente {
        chemin: PathBuf,
        declaree: NaiveDate,
        attendue: NaiveDate,
    },
}

/// Métadonnées d'une journée. Les champs rédactionnels que le pipeline
/// n'utilise pas (étiquettes, dénivelé, temps fort) sont ignorés sans erreur :
/// le frontmatter appartient à la rédaction, pas au pipeline.
#[derive(Debug, Clone, Deserialize)]
pub struct Journee {
    pub date: NaiveDate,
    #[serde(default)]
    pub titre: Option<String>,
    /// Point d'ancrage de la journée : c'est de lui qu'héritent les médias
    /// sans position (D5). Référence un `id` de `voyage.yaml`.
    #[serde(default)]
    pub lieu: Option<String>,
    /// Lieu où l'on a dormi, de type `camp` dans `voyage.yaml`.
    #[serde(default)]
    pub camp: Option<String>,
    #[serde(default)]
    pub couverture: Option<String>,
}

/// Extrait et désérialise le frontmatter d'un fichier Markdown.
///
/// Fonction pure, pour être testable sans toucher au disque.
pub fn lire_frontmatter(texte: &str) -> Option<Result<Journee, serde_norway::Error>> {
    let sans_bom = texte.trim_start_matches('\u{feff}');
    let corps = sans_bom.strip_prefix("---")?;
    let corps = corps.strip_prefix('\n').or_else(|| corps.strip_prefix("\r\n"))?;
    // Le délimiteur de fin est la première ligne réduite à ---.
    let fin = corps
        .lines()
        .scan(0usize, |position, ligne| {
            let debut = *position;
            *position += ligne.len() + 1;
            Some((debut, ligne))
        })
        .find(|(_, ligne)| ligne.trim_end() == "---")
        .map(|(debut, _)| debut)?;
    Some(serde_norway::from_str(&corps[..fin]))
}

/// Charge toutes les journées d'un voyage, triées par date.
///
/// L'absence du dossier `jours/` est légitime : au lot 2, le récit n'est pas
/// encore importé.
pub fn charger(depot: &Path, voyage_id: &str) -> Result<Vec<Journee>, ErreurJours> {
    let dossier = depot
        .join("content")
        .join("voyages")
        .join(voyage_id)
        .join("jours");
    if !dossier.is_dir() {
        return Ok(Vec::new());
    }
    let entrees = std::fs::read_dir(&dossier).map_err(|source| ErreurJours::Lecture {
        chemin: dossier.clone(),
        source,
    })?;

    let mut journees = Vec::new();
    for entree in entrees {
        let chemin = match entree {
            Ok(e) => e.path(),
            Err(source) => {
                return Err(ErreurJours::Lecture {
                    chemin: dossier.clone(),
                    source,
                })
            }
        };
        if chemin.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let texte = std::fs::read_to_string(&chemin).map_err(|source| ErreurJours::Lecture {
            chemin: chemin.clone(),
            source,
        })?;
        let journee = match lire_frontmatter(&texte) {
            Some(Ok(j)) => j,
            Some(Err(source)) => {
                return Err(ErreurJours::Syntaxe {
                    chemin: chemin.clone(),
                    source,
                })
            }
            None => return Err(ErreurJours::FrontmatterAbsent { chemin }),
        };

        // Le nom du fichier est sa date. Un écart entre les deux est une
        // erreur de rédaction qui produirait un récit rangé au mauvais jour.
        if let Some(tige) = chemin.file_stem().and_then(|t| t.to_str()) {
            if let Ok(attendue) = NaiveDate::parse_from_str(tige, "%Y-%m-%d") {
                if attendue != journee.date {
                    return Err(ErreurJours::DateIncoherente {
                        chemin,
                        declaree: journee.date,
                        attendue,
                    });
                }
            }
        }
        journees.push(journee);
    }
    journees.sort_by_key(|j| j.date);
    Ok(journees)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_complet() {
        let texte = "---\ndate: 2026-08-14\ntitre: Lillaz\nlieu: lillaz\ncamp: valnontey\n---\n\nLe récit.\n";
        let journee = lire_frontmatter(texte)
            .expect("frontmatter présent")
            .expect("frontmatter lisible");
        assert_eq!(journee.date, NaiveDate::from_ymd_opt(2026, 8, 14).unwrap());
        assert_eq!(journee.lieu.as_deref(), Some("lillaz"));
        assert_eq!(journee.camp.as_deref(), Some("valnontey"));
    }

    /// Les champs rédactionnels que le pipeline n'utilise pas ne doivent pas
    /// le faire échouer : le frontmatter appartient à la rédaction.
    #[test]
    fn champs_redactionnels_ignores() {
        let texte = "---\ndate: 2026-08-14\netiquettes: [randonnee, fete]\ndistance_marche_km: 14\ntemps_fort: true\n---\nTexte\n";
        assert!(lire_frontmatter(texte).expect("présent").is_ok());
    }

    #[test]
    fn sans_frontmatter() {
        assert!(lire_frontmatter("Juste du texte\n").is_none());
    }

    #[test]
    fn frontmatter_non_termine() {
        assert!(lire_frontmatter("---\ndate: 2026-08-14\nTexte sans fin\n").is_none());
    }

    #[test]
    fn date_obligatoire() {
        let texte = "---\ntitre: sans date\n---\nTexte\n";
        assert!(lire_frontmatter(texte).expect("présent").is_err());
    }
}
