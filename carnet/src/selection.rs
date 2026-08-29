//! Choix des médias publiés. Voir SPEC.md, D7.
//!
//! Le dossier source est une archive, le site est un récit : 833 médias pour
//! 23 journées, ce n'est pas un journal de voyage, c'est une sauvegarde.
//!
//! Trois règles. Tout média cité par une directive du récit est retenu
//! d'office. `retenus` et `exclus` complètent le choix à la main. En
//! l'absence de fichier et de directive, tout est retenu.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::scan::Media;

#[derive(Debug, thiserror::Error)]
pub enum ErreurSelection {
    #[error("lecture de {chemin} impossible")]
    Lecture {
        chemin: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{chemin} est mal formé")]
    Syntaxe {
        chemin: PathBuf,
        #[source]
        source: serde_norway::Error,
    },
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// Médias retenus. Vide, la liste ne restreint rien.
    #[serde(default)]
    pub retenus: Vec<String>,
    /// Médias écartés, quelle que soit leur présence ailleurs.
    #[serde(default)]
    pub exclus: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Bilan {
    pub publies: usize,
    pub ecartes: usize,
    /// Identifiants cités par le récit, donc retenus d'office.
    pub cites_par_le_recit: usize,
    /// Entrées de `selection.yaml` qui ne visent aucun média.
    pub inutilisees: Vec<String>,
    /// Vrai si aucune restriction n'est déclarée : tout est publié.
    pub tout_retenu: bool,
}

/// Relève les identifiants cités par les directives du récit.
///
/// Les directives sont de la forme `::photo{id="..."}` ou
/// `::galerie{ids="a,b,c"}`. La lecture est volontairement tolérante : elle
/// cherche les attributs `id` et `ids`, sans analyser le Markdown.
pub fn identifiants_cites(texte: &str) -> BTreeSet<String> {
    let mut vus = BTreeSet::new();
    for directive in texte.split("::").skip(1) {
        let Some(debut) = directive.find('{') else {
            continue;
        };
        let Some(fin) = directive[debut..].find('}') else {
            continue;
        };
        let mut reste = &directive[debut + 1..debut + fin];

        // La valeur se lit jusqu'au guillemet fermant, et non jusqu'au
        // prochain espace : `ids="a,b, c"` en contient trois, pas deux.
        while let Some(egal) = reste.find('=') {
            let (avant, apres) = reste.split_at(egal);
            let cle = avant
                .rsplit(|c: char| c.is_whitespace())
                .next()
                .unwrap_or("")
                .trim();
            let apres = &apres[1..];

            let (valeur, suite) = match apres.chars().next() {
                Some(guillemet @ ('"' | '\'')) => match apres[1..].find(guillemet) {
                    Some(bout) => (&apres[1..1 + bout], &apres[2 + bout..]),
                    None => (&apres[1..], ""),
                },
                _ => {
                    let bout = apres.find(char::is_whitespace).unwrap_or(apres.len());
                    (&apres[..bout], &apres[bout..])
                }
            };

            if cle == "id" || cle == "ids" {
                for identifiant in valeur.split(',') {
                    let identifiant = identifiant.trim();
                    if !identifiant.is_empty() {
                        vus.insert(identifiant.to_string());
                    }
                }
            }
            reste = suite;
        }
    }
    vus
}

impl Selection {
    /// Charge `content/voyages/<id>/selection.yaml`. Son absence est légitime.
    pub fn charger(depot: &Path, voyage_id: &str) -> Result<Self, ErreurSelection> {
        let chemin = depot
            .join("content")
            .join("voyages")
            .join(voyage_id)
            .join("selection.yaml");
        if !chemin.is_file() {
            return Ok(Self::default());
        }
        let texte = std::fs::read_to_string(&chemin).map_err(|source| ErreurSelection::Lecture {
            chemin: chemin.clone(),
            source,
        })?;
        serde_norway::from_str(&texte).map_err(|source| ErreurSelection::Syntaxe {
            chemin: chemin.clone(),
            source,
        })
    }
}

/// Lit les directives de toutes les journées du voyage.
pub fn citations_du_recit(depot: &Path, voyage_id: &str) -> Result<BTreeSet<String>, ErreurSelection> {
    let dossier = depot
        .join("content")
        .join("voyages")
        .join(voyage_id)
        .join("jours");
    let mut cites = BTreeSet::new();
    if !dossier.is_dir() {
        return Ok(cites);
    }
    let entrees = std::fs::read_dir(&dossier).map_err(|source| ErreurSelection::Lecture {
        chemin: dossier.clone(),
        source,
    })?;
    for entree in entrees.flatten() {
        let chemin = entree.path();
        if chemin.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let texte = std::fs::read_to_string(&chemin).map_err(|source| ErreurSelection::Lecture {
            chemin: chemin.clone(),
            source,
        })?;
        cites.extend(identifiants_cites(&texte));
    }
    Ok(cites)
}

/// Renseigne le champ `publie` de chaque média.
pub fn appliquer(medias: &mut [Media], selection: &Selection, cites: &BTreeSet<String>) -> Bilan {
    let mut bilan = Bilan {
        cites_par_le_recit: cites.len(),
        ..Bilan::default()
    };

    let retenus: BTreeSet<&str> = selection.retenus.iter().map(String::as_str).collect();
    let exclus: BTreeSet<&str> = selection.exclus.iter().map(String::as_str).collect();
    let present: BTreeSet<&str> = medias.iter().map(|m| m.id.as_str()).collect();

    for entree in selection.retenus.iter().chain(selection.exclus.iter()) {
        if !present.contains(entree.as_str()) {
            bilan
                .inutilisees
                .push(format!("« {entree} » : aucun média de ce nom"));
        }
    }

    // Sans liste de retenus ni citation, rien ne restreint : tout est publié.
    bilan.tout_retenu = retenus.is_empty() && cites.is_empty();

    for media in medias.iter_mut() {
        let cite = cites.contains(&media.id);
        let publie = if exclus.contains(media.id.as_str()) {
            false
        } else {
            bilan.tout_retenu || cite || retenus.contains(media.id.as_str())
        };
        media.publie = publie;
        if publie {
            bilan.publies += 1;
        } else {
            bilan.ecartes += 1;
        }
    }
    bilan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noms::Convention;
    use crate::scan::{Fiabilite, OrigineDate, TypeMedia};

    fn media(id: &str) -> Media {
        Media {
            id: id.to_string(),
            type_media: TypeMedia::Photo,
            fichier_source: format!("{id}.jpg"),
            prise_le: None,
            origine_date: OrigineDate::Absente,
            jour: None,
            position: None,
            fiabilite: Fiabilite::Absente,
            origine_position: None,
            lieu: None,
            publie: true,
            derives: None,
            lqip: None,
            anomalies: Vec::new(),
            largeur: None,
            hauteur: None,
            orientation: None,
            appareil: None,
            convention: Convention::Telephone,
            octets: 1,
        }
    }

    #[test]
    fn directives_du_recit() {
        let texte = r#"
Le texte.

::photo{id="IMG001" legende="La montée"}

Encore du texte.

::galerie{ids="IMG002,IMG003, IMG004"}

::video{id=IMG005}
"#;
        let cites = identifiants_cites(texte);
        assert_eq!(cites.len(), 5, "{cites:?}");
        assert!(cites.contains("IMG004"), "le dernier d une liste ne doit pas etre perdu");
        assert!(cites.contains("IMG005"), "une valeur sans guillemets doit etre lue");
        assert!(cites.contains("IMG003"));
    }

    /// D7 : en l'absence de fichier et de citation, tout est retenu.
    #[test]
    fn sans_selection_tout_est_publie() {
        let mut medias = vec![media("A"), media("B")];
        let bilan = appliquer(&mut medias, &Selection::default(), &BTreeSet::new());
        assert!(bilan.tout_retenu);
        assert_eq!(bilan.publies, 2);
        assert!(medias.iter().all(|m| m.publie));
    }

    #[test]
    fn une_liste_de_retenus_restreint() {
        let mut medias = vec![media("A"), media("B"), media("C")];
        let selection = Selection {
            retenus: vec!["A".into(), "C".into()],
            exclus: Vec::new(),
        };
        let bilan = appliquer(&mut medias, &selection, &BTreeSet::new());
        assert_eq!(bilan.publies, 2);
        assert_eq!(bilan.ecartes, 1);
        assert!(!medias.iter().find(|m| m.id == "B").expect("B").publie);
    }

    /// Un média cité par le récit est retenu sans avoir à le répéter.
    #[test]
    fn le_recit_retient_d_office() {
        let mut medias = vec![media("A"), media("B")];
        let cites: BTreeSet<String> = ["B".to_string()].into_iter().collect();
        let bilan = appliquer(&mut medias, &Selection::default(), &cites);
        assert!(!bilan.tout_retenu, "une citation suffit à restreindre");
        assert_eq!(bilan.publies, 1);
        assert!(medias.iter().find(|m| m.id == "B").expect("B").publie);
    }

    /// L'exclusion l'emporte sur tout, y compris sur une citation.
    #[test]
    fn l_exclusion_prime() {
        let mut medias = vec![media("A")];
        let selection = Selection {
            retenus: vec!["A".into()],
            exclus: vec!["A".into()],
        };
        let cites: BTreeSet<String> = ["A".to_string()].into_iter().collect();
        appliquer(&mut medias, &selection, &cites);
        assert!(!medias[0].publie);
    }

    #[test]
    fn entree_sans_cible_signalee() {
        let mut medias = vec![media("A")];
        let selection = Selection {
            retenus: vec!["INEXISTANT".into()],
            exclus: Vec::new(),
        };
        let bilan = appliquer(&mut medias, &selection, &BTreeSet::new());
        assert_eq!(bilan.inutilisees.len(), 1);
    }
}
