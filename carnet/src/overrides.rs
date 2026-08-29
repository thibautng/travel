//! Lecture et application de `content/voyages/<id>/overrides.yaml`.
//!
//! Voir SPEC.md, section 7. C'est la pièce maîtresse de la fiabilité du
//! projet : sans elle, les erreurs GPS deviennent définitives.
//!
//! Règle de priorité : une surcharge écrase l'EXIF, toujours, sans
//! avertissement. En contrepartie, `carnet check` liste ce qui a été appliqué,
//! et **échoue** sur une surcharge qui ne vise rien : une correction qui ne
//! s'applique pas est une correction perdue.

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};
use chrono_tz::Tz;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::scan::{Anomalie, Fiabilite, Media, OriginePosition, Position};
use crate::track::Mode;
use crate::voyage::Position as PositionYaml;

#[derive(Debug, thiserror::Error)]
pub enum ErreurOverrides {
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

    #[error("un lot porte un motif vide, ce qui viserait tous les médias")]
    MotifVide,

    #[error("le segment du {jour} ne porte que {nombre} point, il en faut au moins deux")]
    SegmentTropCourt { jour: NaiveDate, nombre: usize },

    #[error("coordonnée hors limites dans le segment du {jour} : [{lon}, {lat}]")]
    CoordonneeInvalide {
        jour: NaiveDate,
        lon: f64,
        lat: f64,
    },
}

/// Correction visant un média précis.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurchargeMedia {
    #[serde(default)]
    pub position: Option<PositionYaml>,
    #[serde(default)]
    pub prise_le: Option<DateTime<FixedOffset>>,
    #[serde(default)]
    pub jour: Option<NaiveDate>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Correction visant un ensemble de fichiers désignés par un motif.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lot {
    /// Motif appliqué au nom de fichier, extension comprise. `*` et `?`.
    pub motif: String,
    #[serde(default)]
    pub jour: Option<NaiveDate>,
    #[serde(default)]
    pub position: Option<PositionYaml>,
    #[serde(default)]
    pub prise_le: Option<DateTime<FixedOffset>>,
    /// Répartir les médias le long du segment manuel de la journée, plutôt
    /// que de les empiler sur un point unique. Voir C4.
    #[serde(default)]
    pub repartir_sur_segment: bool,
    #[serde(default)]
    pub note: Option<String>,
}

/// Tronçon de trace saisi à la main, là où aucune photo ne documente le trajet.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub jour: NaiveDate,
    pub mode: Mode,
    /// Identifiant du média après lequel insérer le tronçon. À défaut, le
    /// segment est raccroché par l'ordre de ses points.
    #[serde(default)]
    pub apres: Option<String>,
    /// Points en ordre GeoJSON : `[longitude, latitude]`.
    pub points: Vec<[f64; 2]>,
    /// Faire calculer le tracé entre les points par le moteur d'itinéraire,
    /// au lieu de les relier par des droites.
    ///
    /// Utile pour un trajet routier qu'aucune photo ne documente : on déclare
    /// le départ et l'arrivée, le moteur trouve la route. Sans effet sur les
    /// modes non calculables (D6).
    #[serde(default)]
    pub calculer: bool,
    #[serde(default)]
    pub note: Option<String>,
}

/// Mode imposé à une journée, ou à une tranche horaire de journée.
///
/// L'inférence par la vitesse est une proposition, pas un verdict : une
/// voiture arrêtée pour déjeuner affiche la vitesse moyenne d'un vélo. C'est
/// ici que la proposition se corrige.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForcageMode {
    pub jour: NaiveDate,
    pub mode: Mode,
    /// Début de la tranche, en heure locale. À défaut, le début de journée.
    #[serde(default)]
    pub de: Option<NaiveTime>,
    /// Fin de la tranche. À défaut, la fin de journée.
    #[serde(default)]
    pub a: Option<NaiveTime>,
    #[serde(default)]
    pub note: Option<String>,
}

impl ForcageMode {
    fn couvre(&self, jour: NaiveDate, instant: NaiveTime) -> bool {
        self.jour == jour
            && self.de.map(|debut| instant >= debut).unwrap_or(true)
            && self.a.map(|fin| instant <= fin).unwrap_or(true)
    }
}

/// Points de passage imposés à un itinéraire calculé. Voir D6.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForcageItineraire {
    pub jour: NaiveDate,
    #[serde(default = "Mode::route")]
    pub mode: Mode,
    pub points_de_passage: Vec<[f64; 2]>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    #[serde(default)]
    pub medias: BTreeMap<String, SurchargeMedia>,
    #[serde(default)]
    pub exclusions: Vec<String>,
    #[serde(default)]
    pub lots: Vec<Lot>,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub itineraires: Vec<ForcageItineraire>,
    #[serde(default)]
    pub modes: Vec<ForcageMode>,
}

impl Overrides {
    /// Mode imposé pour un instant donné, s'il y en a un.
    ///
    /// La première règle qui couvre l'instant gagne : une tranche horaire
    /// déclarée avant la règle de journée l'emporte donc sur elle.
    pub fn mode_force(&self, jour: NaiveDate, instant: NaiveTime) -> Option<Mode> {
        self.modes
            .iter()
            .find(|f| f.couvre(jour, instant))
            .map(|f| f.mode)
    }
}

/// Médias d'un lot à répartir le long du segment manuel de leur journée.
#[derive(Debug, Clone)]
pub struct Repartition {
    pub jour: NaiveDate,
    /// Identifiants dans leur ordre relatif. Pour les GoPro, l'horodatage est
    /// faux en absolu mais croissant, et le nom sert de repli (C4).
    pub identifiants: Vec<String>,
}

/// Ce que l'application des surcharges a produit, pour `carnet check`.
#[derive(Debug, Default)]
pub struct Journal {
    pub medias_surcharges: Vec<String>,
    pub exclus: Vec<String>,
    /// Motif du lot et nombre de médias touchés.
    pub lots_appliques: Vec<(String, usize)>,
    /// Surcharges qui ne visent aucun média. Fait échouer `carnet check`.
    pub inutilisees: Vec<String>,
    pub repartitions: Vec<Repartition>,
}

impl Journal {
    pub fn total_applique(&self) -> usize {
        self.medias_surcharges.len()
            + self.exclus.len()
            + self.lots_appliques.iter().map(|(_, n)| n).sum::<usize>()
    }
}

/// Compare un nom de fichier à un motif, avec `*` et `?`, sans distinction de
/// casse. Écrit à la main plutôt qu'avec une dépendance : deux jokers
/// suffisent, et le dossier source est sous Windows, donc insensible à la casse.
fn motif_correspond(motif: &str, nom: &str) -> bool {
    let m: Vec<char> = motif.to_lowercase().chars().collect();
    let n: Vec<char> = nom.to_lowercase().chars().collect();

    // Parcours glouton avec retour arrière sur la dernière étoile rencontrée.
    let (mut i, mut j) = (0usize, 0usize);
    let (mut etoile, mut reprise) = (None, 0usize);
    while j < n.len() {
        if i < m.len() && (m[i] == '?' || m[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < m.len() && m[i] == '*' {
            etoile = Some(i);
            reprise = j;
            i += 1;
        } else if let Some(e) = etoile {
            i = e + 1;
            reprise += 1;
            j = reprise;
        } else {
            return false;
        }
    }
    while i < m.len() && m[i] == '*' {
        i += 1;
    }
    i == m.len()
}

fn en_position(p: &PositionYaml) -> Position {
    Position {
        lat: p.lat,
        lon: p.lon,
        alt: p.alt,
    }
}

fn coordonnee_valide(point: &[f64; 2]) -> bool {
    (-180.0..=180.0).contains(&point[0]) && (-90.0..=90.0).contains(&point[1])
}

impl Overrides {
    /// Charge `content/voyages/<id>/overrides.yaml`.
    ///
    /// L'absence du fichier est légitime : un voyage sans correction est un
    /// voyage sans `overrides.yaml`.
    pub fn charger(depot: &Path, voyage_id: &str) -> Result<Self, ErreurOverrides> {
        let chemin = depot
            .join("content")
            .join("voyages")
            .join(voyage_id)
            .join("overrides.yaml");
        if !chemin.is_file() {
            return Ok(Self::default());
        }
        let texte = std::fs::read_to_string(&chemin).map_err(|source| ErreurOverrides::Lecture {
            chemin: chemin.clone(),
            source,
        })?;
        let overrides: Overrides =
            serde_norway::from_str(&texte).map_err(|source| ErreurOverrides::Syntaxe {
                chemin: chemin.clone(),
                source,
            })?;
        overrides.valider()?;
        Ok(overrides)
    }

    /// Contrôles qui ne dépendent d'aucun média : un fichier incohérent doit
    /// être refusé avant d'avoir touché quoi que ce soit.
    fn valider(&self) -> Result<(), ErreurOverrides> {
        for lot in &self.lots {
            if lot.motif.trim().is_empty() {
                return Err(ErreurOverrides::MotifVide);
            }
        }
        for segment in &self.segments {
            if segment.points.len() < 2 {
                return Err(ErreurOverrides::SegmentTropCourt {
                    jour: segment.jour,
                    nombre: segment.points.len(),
                });
            }
            for point in &segment.points {
                if !coordonnee_valide(point) {
                    return Err(ErreurOverrides::CoordonneeInvalide {
                        jour: segment.jour,
                        lon: point[0],
                        lat: point[1],
                    });
                }
            }
        }
        for forcage in &self.itineraires {
            for point in &forcage.points_de_passage {
                if !coordonnee_valide(point) {
                    return Err(ErreurOverrides::CoordonneeInvalide {
                        jour: forcage.jour,
                        lon: point[0],
                        lat: point[1],
                    });
                }
            }
        }
        Ok(())
    }

    /// Applique les surcharges à l'inventaire.
    ///
    /// Ordre : exclusions, puis lots, puis corrections individuelles. Une
    /// correction individuelle l'emporte donc sur le lot qui la couvrirait,
    /// ce qui permet de traiter l'exception sans défaire la règle.
    pub fn appliquer(&self, medias: &mut Vec<Media>, fuseau: Tz) -> Journal {
        let mut journal = Journal::default();

        // 1. Exclusions.
        let exclus: BTreeSet<&str> = self.exclusions.iter().map(String::as_str).collect();
        let present: BTreeSet<String> = medias.iter().map(|m| m.id.clone()).collect();
        medias.retain(|m| !exclus.contains(m.id.as_str()));
        for id in &self.exclusions {
            if present.contains(id) {
                journal.exclus.push(id.clone());
            } else {
                journal
                    .inutilisees
                    .push(format!("exclusion « {id} » : aucun média de ce nom"));
            }
        }

        // 2. Lots, désignés par motif sur le nom de fichier.
        for lot in &self.lots {
            let mut touches: Vec<(Option<DateTime<FixedOffset>>, String, String)> = Vec::new();
            for media in medias.iter_mut() {
                let nom = nom_de_fichier(&media.fichier_source);
                if !motif_correspond(&lot.motif, nom) {
                    continue;
                }
                if let Some(prise_le) = lot.prise_le {
                    media.prise_le = Some(prise_le);
                    media.jour = Some(prise_le.with_timezone(&fuseau).date_naive());
                }
                // `jour` sans `prise_le` corrige la journée en gardant
                // l'horodatage d'origine : faux en absolu, mais c'est lui qui
                // porte l'ordre relatif des médias du lot (C4).
                if let Some(jour) = lot.jour {
                    media.jour = Some(jour);
                }
                if let Some(position) = &lot.position {
                    media.position = Some(en_position(position));
                    media.fiabilite = Fiabilite::Haute;
                    media.origine_position = Some(OriginePosition::Override);
                    media.anomalies.retain(|a| *a != Anomalie::AltitudeNulle);
                }
                touches.push((media.prise_le, nom.to_string(), media.id.clone()));
            }
            if touches.is_empty() {
                journal.inutilisees.push(format!(
                    "lot « {} » : aucun fichier ne correspond",
                    lot.motif
                ));
                continue;
            }
            journal
                .lots_appliques
                .push((lot.motif.clone(), touches.len()));

            if lot.repartir_sur_segment {
                // Ordre relatif : horodatage d'abord, nom de fichier ensuite.
                touches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                journal.repartitions.push(Repartition {
                    jour: lot.jour.unwrap_or_default(),
                    identifiants: touches.into_iter().map(|(_, _, id)| id).collect(),
                });
            }
        }

        // 3. Corrections individuelles.
        for (id, surcharge) in &self.medias {
            let Some(media) = medias.iter_mut().find(|m| &m.id == id) else {
                journal
                    .inutilisees
                    .push(format!("surcharge « {id} » : aucun média de ce nom"));
                continue;
            };
            if let Some(prise_le) = surcharge.prise_le {
                media.prise_le = Some(prise_le);
                media.jour = Some(prise_le.with_timezone(&fuseau).date_naive());
                media.anomalies.retain(|a| *a != Anomalie::NomMenteur);
            }
            if let Some(jour) = surcharge.jour {
                media.jour = Some(jour);
            }
            if let Some(position) = &surcharge.position {
                media.position = Some(en_position(position));
                media.fiabilite = Fiabilite::Haute;
                media.origine_position = Some(OriginePosition::Override);
                media.anomalies.retain(|a| *a != Anomalie::AltitudeNulle);
            }
            journal.medias_surcharges.push(id.clone());
        }

        journal
    }
}

fn nom_de_fichier(chemin_relatif: &str) -> &str {
    chemin_relatif
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(chemin_relatif)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noms::Convention;
    use crate::scan::{OrigineDate, TypeMedia};

    fn media(id: &str, fichier: &str) -> Media {
        Media {
            id: id.to_string(),
            type_media: TypeMedia::Photo,
            fichier_source: fichier.to_string(),
            prise_le: DateTime::parse_from_rfc3339("2016-01-03T10:00:00+01:00").ok(),
            origine_date: OrigineDate::Exif,
            jour: NaiveDate::from_ymd_opt(2016, 1, 3),
            position: None,
            fiabilite: Fiabilite::Absente,
            origine_position: None,
            lieu: None,
            publie: true,
            derives: None,
            lqip: None,
            anomalies: vec![Anomalie::HorlogePerdue],
            largeur: None,
            hauteur: None,
            orientation: None,
            appareil: None,
            convention: Convention::GoPro,
            octets: 1000,
        }
    }

    fn fuseau() -> Tz {
        chrono_tz::Europe::Paris
    }

    #[test]
    fn motifs_avec_jokers() {
        assert!(motif_correspond("GOPR2*.JPG", "GOPR2699.JPG"));
        assert!(motif_correspond("GOPR2*.JPG", "gopr2717.jpg"));
        assert!(!motif_correspond("GOPR2*.JPG", "GOPR3599.JPG"));
        assert!(motif_correspond("IMG2026080?*.jpg", "IMG20260808113008.jpg"));
        assert!(motif_correspond("*", "n'importe quoi"));
        assert!(!motif_correspond("*.mp4", "photo.jpg"));
        assert!(motif_correspond("IMG*08*.jpg", "IMG20260808113008.jpg"));
    }

    /// Le motif `GOPR27*` de l'exemple de la spec 1.2 ratait `GOPR2699`,
    /// premier des quatorze fichiers GoPro. Un motif trop étroit ne prévient
    /// pas : il applique la correction à une partie du lot seulement.
    #[test]
    fn motif_trop_etroit_rate_une_partie_du_lot() {
        assert!(!motif_correspond("GOPR27*.JPG", "GOPR2699.JPG"));
        assert!(motif_correspond("GOPR27*.JPG", "GOPR2700.JPG"));
    }

    /// C4 : les 14 GoPro sont redatées et repositionnées en bloc.
    #[test]
    fn lot_applique_jour_et_position() {
        let mut medias = vec![media("GOPR2699", "GOPR2699.JPG"), media("GOPR2700", "GOPR2700.JPG")];
        let overrides: Overrides = serde_norway::from_str(
            r#"
lots:
  - motif: "GOPR2*.JPG"
    jour: 2026-08-02
    position: { lat: 46.2010, lon: 13.6480 }
    repartir_sur_segment: true
    note: "Canyoning du Sušec"
"#,
        )
        .expect("yaml lisible");
        let journal = overrides.appliquer(&mut medias, fuseau());

        assert_eq!(journal.lots_appliques, vec![("GOPR2*.JPG".to_string(), 2)]);
        assert!(journal.inutilisees.is_empty());
        for m in &medias {
            assert_eq!(m.jour, NaiveDate::from_ymd_opt(2026, 8, 2));
            assert_eq!(m.fiabilite, Fiabilite::Haute);
            assert_eq!(m.origine_position, Some(OriginePosition::Override));
        }
        // L'horodatage d'origine est conservé : il porte l'ordre relatif.
        assert!(medias[0].prise_le.is_some());
        assert_eq!(journal.repartitions.len(), 1);
        assert_eq!(journal.repartitions[0].identifiants.len(), 2);
    }

    #[test]
    fn correction_individuelle_prime_sur_le_lot() {
        let mut medias = vec![media("GOPR2699", "GOPR2699.JPG")];
        let overrides: Overrides = serde_norway::from_str(
            r#"
lots:
  - motif: "GOPR*.JPG"
    position: { lat: 46.2010, lon: 13.6480 }
medias:
  GOPR2699:
    position: { lat: 46.3000, lon: 13.7000, alt: 500 }
"#,
        )
        .expect("yaml lisible");
        overrides.appliquer(&mut medias, fuseau());
        let position = medias[0].position.expect("position posée");
        assert!((position.lat - 46.3).abs() < 1e-9);
        assert_eq!(position.alt, Some(500.0));
    }

    #[test]
    fn exclusion_retire_le_media() {
        let mut medias = vec![media("A", "A.jpg"), media("B", "B.jpg")];
        let overrides: Overrides =
            serde_norway::from_str("exclusions: [A]").expect("yaml lisible");
        let journal = overrides.appliquer(&mut medias, fuseau());
        assert_eq!(medias.len(), 1);
        assert_eq!(medias[0].id, "B");
        assert_eq!(journal.exclus, vec!["A".to_string()]);
    }

    /// Une correction qui ne s'applique à rien est une correction perdue.
    #[test]
    fn surcharge_sans_cible_est_signalee() {
        let mut medias = vec![media("A", "A.jpg")];
        let overrides: Overrides = serde_norway::from_str(
            r#"
exclusions: [INEXISTANT]
lots:
  - motif: "RIEN*.jpg"
    jour: 2026-08-02
medias:
  AUTRE_INEXISTANT:
    jour: 2026-08-02
"#,
        )
        .expect("yaml lisible");
        let journal = overrides.appliquer(&mut medias, fuseau());
        assert_eq!(journal.inutilisees.len(), 3);
    }

    /// L'inférence par la vitesse est une proposition : c'est ici qu'elle se
    /// corrige, à la journée ou à la tranche horaire.
    #[test]
    fn mode_force_par_journee_et_par_tranche() {
        let overrides: Overrides = serde_norway::from_str(
            r#"
modes:
  - jour: 2026-08-06
    mode: velo
    de: "13:30"
    a: "18:00"
    note: "Tassenbach vers Lienz"
  - jour: 2026-08-06
    mode: route
  - jour: 2026-08-12
    mode: route
"#,
        )
        .expect("yaml lisible");
        let jour = NaiveDate::from_ymd_opt(2026, 8, 6).expect("date");
        let heure = |h, m| NaiveTime::from_hms_opt(h, m, 0).expect("heure");

        // La tranche horaire est déclarée en premier, elle l'emporte.
        assert_eq!(overrides.mode_force(jour, heure(14, 0)), Some(Mode::Velo));
        // Hors tranche, la règle de journée s'applique.
        assert_eq!(overrides.mode_force(jour, heure(9, 0)), Some(Mode::Route));
        // Une journée sans règle laisse l'inférence décider.
        let autre = NaiveDate::from_ymd_opt(2026, 8, 7).expect("date");
        assert_eq!(overrides.mode_force(autre, heure(9, 0)), None);
    }

    #[test]
    fn cle_inconnue_refusee() {
        let erreur = serde_norway::from_str::<Overrides>("medais: {}");
        assert!(erreur.is_err(), "une clé mal orthographiée doit être refusée");
    }

    #[test]
    fn segment_trop_court_refuse() {
        let overrides: Overrides = serde_norway::from_str(
            r#"
segments:
  - jour: 2026-07-31
    mode: bateau
    points: [[12.9887, 47.5893]]
"#,
        )
        .expect("yaml lisible");
        assert!(overrides.valider().is_err());
    }

    #[test]
    fn coordonnee_inversee_refusee() {
        // Latitude en première position : erreur classique, GeoJSON attend
        // [longitude, latitude].
        let overrides: Overrides = serde_norway::from_str(
            r#"
segments:
  - jour: 2026-07-31
    mode: bateau
    points: [[47.5893, 112.9887], [47.5225, 12.9797]]
"#,
        )
        .expect("yaml lisible");
        assert!(overrides.valider().is_err());
    }
}
