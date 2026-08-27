//! Parcours du dossier source et lecture des métadonnées.
//!
//! Voir SPEC.md, section 6.2, étapes 1 et 2.
//!
//! Le dossier source est en LECTURE SEULE. Ce module n'écrit rien.

use crate::noms::{self, Collision, Convention, NomAnalyse};
use crate::voyage::Voyage;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use indicatif::{ProgressBar, ProgressStyle};
use nom_exif::{ExifTag, MediaKind, MediaParser, MediaSource, TrackInfoTag};
use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, thiserror::Error)]
pub enum ErreurScan {
    #[error(
        "{nombre} collisions d'identifiants. Aucun arbitrage automatique n'est fait : \
         corriger dans overrides.yaml ou ajouter le dossier à dossiers_ignores.\n{detail}"
    )]
    Collisions { nombre: usize, detail: String },

    #[error("le dossier source {0} est illisible")]
    Parcours(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeMedia {
    Photo,
    Video,
}

/// D'où vient la date de prise de vue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OrigineDate {
    /// Cas normal : l'EXIF fait foi (C3).
    Exif,
    /// Dernier recours : aucun EXIF lisible, le nom porte la date (C10).
    Nom,
    Absente,
}

/// D'où vient la position. Voir SPEC.md, section 5.4.
///
/// Le lot 1 ne produit que `Exif`. Les trois autres valeurs arrivent au lot 2,
/// avec les surcharges, l'héritage depuis les lieux et l'interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum OriginePosition {
    Exif,
    Override,
    Heritee,
    Interpolee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fiabilite {
    Haute,
    Basse,
    Absente,
}

/// Motif documenté d'une suspicion. Voir SPEC.md, section 5.4.
///
/// `Homonyme` n'est pas posé au lot 1 : une collision d'identifiant fait
/// échouer `carnet scan` avant d'en arriver là. Elle ne deviendra une simple
/// anomalie qu'au lot 2, quand `overrides.yaml` pourra la résoudre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum Anomalie {
    /// C1 : altitude absente ou strictement nulle, donc position réseau.
    AltitudeNulle,
    /// C2 : coordonnées identiques à un autre média, à plus de 20 minutes.
    PositionClonee,
    /// C3 : le nom porte une date différente de celle de l'EXIF.
    NomMenteur,
    /// C4 : horloge de l'appareil manifestement fausse.
    HorlogePerdue,
    /// C8 : homonyme d'un autre fichier du dossier source.
    Homonyme,
    /// C6 : l'identifiant diffère du nom d'origine.
    NomNormalise,
    /// C10 : aucun bloc EXIF dans le fichier.
    ExifAbsent,
    /// C10 : date lue dans le nom, faute d'EXIF.
    DateDuNom,
    /// C9 : bloc GPS sans référence d'hémisphère lisible.
    HemisphereAbsent,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Position {
    pub lat: f64,
    pub lon: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<f64>,
}

/// Un média inventorié. Sérialisé tel quel dans `media.json`.
#[derive(Debug, Clone, Serialize)]
pub struct Media {
    pub id: String,
    #[serde(rename = "type")]
    pub type_media: TypeMedia,
    pub fichier_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prise_le: Option<DateTime<FixedOffset>>,
    pub origine_date: OrigineDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jour: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    pub fiabilite: Fiabilite,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origine_position: Option<OriginePosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lieu: Option<String>,
    pub anomalies: Vec<Anomalie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largeur: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hauteur: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appareil: Option<String>,

    /// Métadonnées internes au pipeline, utiles aux statistiques.
    #[serde(skip)]
    pub convention: Convention,
    #[serde(skip)]
    pub octets: u64,
}

pub struct Inventaire {
    pub medias: Vec<Media>,
    /// Fichiers écartés parce qu'ils ne sont pas des médias.
    pub non_medias: usize,
    /// Dossiers sautés au titre de `dossiers_ignores` (C8).
    pub dossiers_sautes: Vec<PathBuf>,
}

/// Ancre un horodatage EXIF dans le temps.
///
/// Si l'EXIF portait `OffsetTimeOriginal`, cet offset fait foi. Sinon,
/// l'horodatage est interprété dans le fuseau du voyage. Voir SPEC.md,
/// section 6.2, étape 2.
fn ancrer(naif: NaiveDateTime, offset_connu: Option<FixedOffset>, fuseau: Tz) -> DateTime<FixedOffset> {
    if let Some(offset) = offset_connu {
        if let Some(dt) = naif.and_local_timezone(offset).single() {
            return dt;
        }
    }
    match fuseau.from_local_datetime(&naif).earliest() {
        Some(dt) => dt.fixed_offset(),
        // Heure inexistante (passage à l'heure d'été) : on prend l'instant
        // équivalent plutôt que de perdre le média.
        None => naif.and_utc().with_timezone(&fuseau).fixed_offset(),
    }
}

#[derive(Default)]
struct LectureExif {
    prise_le: Option<NaiveDateTime>,
    offset: Option<FixedOffset>,
    position: Option<Position>,
    hemisphere_absent: bool,
    appareil: Option<String>,
    largeur: Option<u32>,
    hauteur: Option<u32>,
    orientation: Option<u16>,
    presente: bool,
}

/// Lit les métadonnées d'un fichier. Une lecture impossible n'est pas une
/// erreur : c'est un fichier sans EXIF, cas nominal pour C10.
fn lire_metadonnees(parser: &mut MediaParser, chemin: &Path, video: bool) -> LectureExif {
    let mut lecture = LectureExif::default();

    let Ok(source) = MediaSource::open(chemin) else {
        return lecture;
    };

    if video || matches!(source.kind(), MediaKind::Track) {
        let Ok(piste) = parser.parse_track(source) else {
            return lecture;
        };
        lecture.presente = true;
        if let Some(date) = piste
            .get(TrackInfoTag::CreateDate)
            .and_then(|v| v.as_datetime())
        {
            lecture.offset = date.aware().map(|d| *d.offset());
            lecture.prise_le = Some(date.into_naive());
        }
        // Les accesseurs décimaux appliquent eux-mêmes les références
        // d'hémisphère et le signe de l'altitude (C9).
        if let Some(gps) = piste.gps_info() {
            if let (Some(lat), Some(lon)) = (gps.latitude_decimal(), gps.longitude_decimal()) {
                lecture.position = Some(Position {
                    lat,
                    lon,
                    alt: gps.altitude_meters(),
                });
            }
        }
        return lecture;
    }

    let Ok(iter) = parser.parse_exif(source) else {
        return lecture;
    };
    let exif: nom_exif::Exif = iter.into();
    lecture.presente = true;

    if let Some(date) = exif
        .get(ExifTag::DateTimeOriginal)
        .and_then(|v| v.as_datetime())
    {
        lecture.offset = date.aware().map(|d| *d.offset());
        lecture.prise_le = Some(date.into_naive());
    }

    if let Some(gps) = exif.gps_info() {
        match (gps.latitude_decimal(), gps.longitude_decimal()) {
            (Some(lat), Some(lon)) => {
                lecture.position = Some(Position {
                    lat,
                    lon,
                    alt: gps.altitude_meters(),
                });
            }
            _ => lecture.hemisphere_absent = true,
        }
    }

    lecture.appareil = exif
        .get(ExifTag::Model)
        .and_then(|v| v.as_str())
        .map(|modele| {
            match exif.get(ExifTag::Make).and_then(|v| v.as_str()) {
                Some(marque) if !modele.starts_with(marque) => {
                    format!("{} {}", marque.trim(), modele.trim())
                }
                _ => modele.trim().to_string(),
            }
        });

    lecture.largeur = exif
        .get(ExifTag::ExifImageWidth)
        .or_else(|| exif.get(ExifTag::ImageWidth))
        .and_then(|v| v.as_u32());
    lecture.hauteur = exif
        .get(ExifTag::ExifImageHeight)
        .or_else(|| exif.get(ExifTag::ImageHeight))
        .and_then(|v| v.as_u32());
    lecture.orientation = exif.get(ExifTag::Orientation).and_then(|v| v.as_u16());

    lecture
}

/// Assemble un média à partir de son nom et de ses métadonnées.
fn construire(
    chemin: &Path,
    relatif: &Path,
    nom: NomAnalyse,
    lecture: LectureExif,
    octets: u64,
    fuseau: Tz,
) -> Media {
    let mut anomalies = Vec::new();
    if nom.normalise {
        anomalies.push(Anomalie::NomNormalise);
    }
    if !lecture.presente {
        anomalies.push(Anomalie::ExifAbsent);
    }
    if lecture.hemisphere_absent {
        anomalies.push(Anomalie::HemisphereAbsent);
    }

    // Datation : EXIF, puis nom, puis rien. Jamais la date de modification
    // du fichier, qui ne survit pas aux copies (SPEC.md, section 6.2).
    let (prise_le, origine_date) = match lecture.prise_le {
        Some(naif) => (
            Some(ancrer(naif, lecture.offset, fuseau)),
            OrigineDate::Exif,
        ),
        None => match nom.horodatage {
            Some(naif) => {
                anomalies.push(Anomalie::DateDuNom);
                (Some(ancrer(naif, None, fuseau)), OrigineDate::Nom)
            }
            None => (None, OrigineDate::Absente),
        },
    };

    // C3 : le nom porte une date, l'EXIF en porte une autre. L'EXIF gagne,
    // mais l'écart est signalé.
    //
    // La comparaison porte sur le jour, pas sur la seconde, et se fait dans
    // le fuseau du voyage. Deux raisons. Les vidéos MP4 horodatent en UTC :
    // comparer leur instant brut à un nom écrit en heure locale signalerait
    // les 128 vidéos du voyage. Et leur CreateDate est la fin de la prise,
    // pas son début, d'où quelques dizaines de secondes d'écart normales.
    // Les cas visés par C3 sont d'un tout autre ordre : deux jours d'écart.
    if origine_date == OrigineDate::Exif && nom.convention.porte_une_date() {
        if let (Some(du_nom), Some(reelle)) = (nom.horodatage, prise_le) {
            if reelle.with_timezone(&fuseau).date_naive() != du_nom.date() {
                anomalies.push(Anomalie::NomMenteur);
            }
        }
    }

    let jour = prise_le.map(|d| d.with_timezone(&fuseau).date_naive());

    let type_media = if noms::est_video(chemin) {
        TypeMedia::Video
    } else {
        TypeMedia::Photo
    };

    Media {
        id: nom.identifiant,
        type_media,
        fichier_source: relatif.to_string_lossy().replace('\\', "/"),
        prise_le,
        origine_date,
        jour,
        position: lecture.position,
        // La fiabilité est décidée par le module quality.
        fiabilite: Fiabilite::Absente,
        origine_position: lecture.position.map(|_| OriginePosition::Exif),
        lieu: None,
        anomalies,
        largeur: lecture.largeur,
        hauteur: lecture.hauteur,
        orientation: lecture.orientation,
        appareil: lecture.appareil,
        convention: nom.convention,
        octets,
    }
}

/// Inventorie le dossier source du voyage.
pub fn inventorier(voyage: &Voyage, silencieux: bool) -> Result<Inventaire, ErreurScan> {
    let racine = &voyage.source_photos;
    let mut fichiers: Vec<(PathBuf, PathBuf, NomAnalyse, u64)> = Vec::new();
    let mut non_medias = 0usize;
    let mut dossiers_sautes = Vec::new();

    let parcours = WalkDir::new(racine).into_iter().filter_entry(|e| {
        if e.depth() == 0 || !e.file_type().is_dir() {
            return true;
        }
        !voyage.dossier_ignore(&e.file_name().to_string_lossy())
    });

    for entree in parcours {
        let Ok(entree) = entree else {
            return Err(ErreurScan::Parcours(racine.clone()));
        };
        if entree.file_type().is_dir() {
            if entree.depth() > 0 && voyage.dossier_ignore(&entree.file_name().to_string_lossy()) {
                dossiers_sautes.push(entree.path().to_path_buf());
            }
            continue;
        }
        let chemin = entree.path();
        if !noms::est_media(chemin) {
            non_medias += 1;
            continue;
        }
        let nom_fichier = entree.file_name().to_string_lossy().to_string();
        let relatif = chemin.strip_prefix(racine).unwrap_or(chemin).to_path_buf();
        let octets = entree.metadata().map(|m| m.len()).unwrap_or(0);
        fichiers.push((
            chemin.to_path_buf(),
            relatif,
            noms::analyser(&nom_fichier),
            octets,
        ));
    }

    // C8 : une collision d'identifiant fait échouer la commande, sans
    // arbitrage automatique. Voir SPEC.md, section 5.3.
    let entrees: Vec<(String, PathBuf)> = fichiers
        .iter()
        .map(|(chemin, _, nom, _)| (nom.identifiant.clone(), chemin.clone()))
        .collect();
    let collisions = noms::detecter_collisions(&entrees);
    if !collisions.is_empty() {
        return Err(ErreurScan::Collisions {
            nombre: collisions.len(),
            detail: format_collisions(&collisions),
        });
    }

    let barre = if silencieux {
        ProgressBar::hidden()
    } else {
        let b = ProgressBar::new(fichiers.len() as u64);
        if let Ok(style) = ProgressStyle::with_template(
            "  lecture EXIF {bar:32} {pos}/{len} fichiers ({eta})",
        ) {
            b.set_style(style);
        }
        b
    };

    let fuseau = voyage.fuseau;
    let mut medias: Vec<Media> = fichiers
        .par_iter()
        .map_init(MediaParser::new, |parser, (chemin, relatif, nom, octets)| {
            let lecture = lire_metadonnees(parser, chemin, noms::est_video(chemin));
            barre.inc(1);
            construire(chemin, relatif, nom.clone(), lecture, *octets, fuseau)
        })
        .collect();
    barre.finish_and_clear();

    medias.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(Inventaire {
        medias,
        non_medias,
        dossiers_sautes,
    })
}

fn format_collisions(collisions: &[Collision]) -> String {
    let mut texte = String::new();
    for c in collisions.iter().take(10) {
        texte.push_str(&format!("  « {} » :\n", c.identifiant));
        for f in &c.fichiers {
            texte.push_str(&format!("    {}\n", f.display()));
        }
    }
    if collisions.len() > 10 {
        texte.push_str(&format!("  ... et {} autres\n", collisions.len() - 10));
    }
    texte
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn naif(a: i32, m: u32, j: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(a, m, j)
            .and_then(|d| NaiveTime::from_hms_opt(h, mi, 0).map(|t| d.and_time(t)))
            .expect("date de test valide")
    }

    #[test]
    fn ancrage_sans_offset_utilise_le_fuseau_du_voyage() {
        let dt = ancrer(naif(2026, 8, 14, 15, 16), None, chrono_tz::Europe::Paris);
        // CEST en août : UTC+2.
        assert_eq!(dt.offset().local_minus_utc(), 2 * 3600);
        assert_eq!(dt.naive_local(), naif(2026, 8, 14, 15, 16));
    }

    #[test]
    fn ancrage_respecte_offset_exif() {
        let offset = FixedOffset::east_opt(9 * 3600).expect("offset valide");
        let dt = ancrer(naif(2024, 10, 19, 11, 40), Some(offset), chrono_tz::Europe::Paris);
        assert_eq!(dt.offset().local_minus_utc(), 9 * 3600);
    }
}
