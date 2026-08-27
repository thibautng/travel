//! Lecture de `content/voyages/<id>/voyage.yaml`.
//!
//! Voir SPEC.md, section 5.1.

use chrono::NaiveDate;
use chrono_tz::Tz;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ErreurVoyage {
    #[error("aucun voyage « {id} » : {chemin} est introuvable")]
    Introuvable { id: String, chemin: PathBuf },

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

    #[error("le dossier source « {0} » est introuvable")]
    SourceIntrouvable(PathBuf),

    #[error("l'identifiant déclaré « {declare} » ne correspond pas au dossier « {dossier} »")]
    IdentifiantIncoherent { declare: String, dossier: String },
}

/// Un lieu géolocalisé du voyage. Voir D5.
///
/// Les lieux ne sont pas exploités au lot 1 : ils portent l'héritage de
/// position, qui arrive au lot 2.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Lieu {
    pub id: String,
    pub nom: String,
    #[serde(default = "TypeLieu::etape")]
    pub type_lieu: TypeLieu,
    pub position: Position,
    #[serde(default)]
    pub du: Option<NaiveDate>,
    #[serde(default)]
    pub au: Option<NaiveDate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeLieu {
    /// On y dort. Marqueur distinct sur la carte.
    Camp,
    Etape,
}

impl TypeLieu {
    fn etape() -> Self {
        TypeLieu::Etape
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[allow(dead_code)]
pub struct Position {
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub alt: Option<f64>,
}

/// Les champs rédactionnels (sous_titre, pays, distance_km, nuits, mode,
/// notion) sont lus par le site au lot 4, pas par le pipeline. Ils sont
/// néanmoins désérialisés ici pour que `carnet check` valide le fichier
/// entier, et non la seule part qui l'intéresse.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Voyage {
    pub id: String,
    pub titre: String,
    #[serde(default)]
    pub sous_titre: Option<String>,
    pub date_debut: NaiveDate,
    pub date_fin: NaiveDate,
    #[serde(default)]
    pub pays: Vec<String>,
    #[serde(default)]
    pub distance_km: Option<u32>,
    #[serde(default)]
    pub nuits: Option<u32>,
    #[serde(default)]
    pub mode: Option<String>,

    /// Heure locale de référence du voyage. Obligatoire : sans lui, le
    /// rattachement d'un média à sa journée civile n'a pas de sens.
    pub fuseau: Tz,

    /// Dossier source, en lecture seule.
    pub source_photos: PathBuf,

    /// Sous-dossiers à ne pas parcourir. Contrainte C8.
    #[serde(default)]
    pub dossiers_ignores: Vec<String>,

    #[serde(default)]
    pub notion: Option<String>,

    #[serde(default)]
    pub lieux: Vec<Lieu>,
}

impl Voyage {
    /// Charge le voyage `id` depuis `<depot>/content/voyages/<id>/voyage.yaml`.
    pub fn charger(depot: &Path, id: &str) -> Result<Self, ErreurVoyage> {
        let chemin = depot
            .join("content")
            .join("voyages")
            .join(id)
            .join("voyage.yaml");
        if !chemin.is_file() {
            return Err(ErreurVoyage::Introuvable {
                id: id.to_string(),
                chemin,
            });
        }
        let texte = std::fs::read_to_string(&chemin).map_err(|source| ErreurVoyage::Lecture {
            chemin: chemin.clone(),
            source,
        })?;
        let voyage: Voyage =
            serde_norway::from_str(&texte).map_err(|source| ErreurVoyage::Syntaxe {
                chemin: chemin.clone(),
                source,
            })?;

        if voyage.id != id {
            return Err(ErreurVoyage::IdentifiantIncoherent {
                declare: voyage.id,
                dossier: id.to_string(),
            });
        }
        if !voyage.source_photos.is_dir() {
            return Err(ErreurVoyage::SourceIntrouvable(voyage.source_photos));
        }
        Ok(voyage)
    }

    /// Vrai si ce nom de dossier doit être sauté au parcours.
    pub fn dossier_ignore(&self, nom: &str) -> bool {
        self.dossiers_ignores.iter().any(|d| d == nom)
    }
}
