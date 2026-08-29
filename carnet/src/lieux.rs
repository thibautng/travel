//! Position des médias que l'EXIF ne renseigne pas.
//!
//! Trois mécanismes, dans cet ordre de préséance :
//!
//! 1. la fiabilité des vidéos, héritée de la photo fiable la plus proche (C1) ;
//! 2. l'interpolation temporelle entre deux positions fiables (section 6.2) ;
//! 3. l'héritage depuis le lieu de la journée (D5).
//!
//! L'interpolation prime sur l'héritage, qui est plus grossier. Aucun des
//! trois ne produit jamais une position `haute` : une position reconstituée
//! reste une position reconstituée.

use chrono::NaiveDate;
use std::collections::BTreeMap;

use crate::jours::Journee;
use crate::scan::{Fiabilite, Media, OriginePosition, Position, TypeMedia};
use crate::voyage::Voyage;

/// Écart maximal, en minutes, entre une vidéo et la photo dont elle hérite
/// la fiabilité. Voir C1, clause vidéo.
const MINUTES_PROMOTION_VIDEO: i64 = 10;

/// Distance maximale, en mètres, entre une vidéo et cette même photo.
///
/// Sans cette seconde condition, une vidéo dont la position est gelée depuis
/// une heure serait promue par une photo prise au même moment ailleurs.
const METRES_PROMOTION_VIDEO: f64 = 500.0;

/// Écart maximal, en minutes, entre un média interpolé et chacune de ses bornes.
const MINUTES_INTERPOLATION: i64 = 30;

/// Distance maximale, en mètres, entre les deux bornes d'une interpolation.
const METRES_INTERPOLATION: f64 = 5_000.0;

#[derive(Debug, Default)]
pub struct Bilan {
    pub videos_promues: usize,
    pub interpolees: usize,
    pub heritees: usize,
    /// Médias qui restent sans position après les trois mécanismes.
    pub sans_position: usize,
    /// Journées dont les médias auraient pu hériter, faute de lieu déclaré.
    pub jours_sans_lieu: Vec<NaiveDate>,
}

/// Distance orthodromique en mètres.
///
/// Écrite à la main : c'est le seul calcul géodésique du lot 2, et il tient
/// en dix lignes. Ajouter `geo` pour cela seul contreviendrait à la règle des
/// dépendances (SPEC.md, section 12).
pub fn distance_m(a: &Position, b: &Position) -> f64 {
    const RAYON_TERRE_M: f64 = 6_371_008.8;
    let (phi1, phi2) = (a.lat.to_radians(), b.lat.to_radians());
    let delta_phi = (b.lat - a.lat).to_radians();
    let delta_lambda = (b.lon - a.lon).to_radians();
    let h = (delta_phi / 2.0).sin().powi(2)
        + phi1.cos() * phi2.cos() * (delta_lambda / 2.0).sin().powi(2);
    2.0 * RAYON_TERRE_M * h.sqrt().asin()
}

/// Applique les trois mécanismes. Les médias sont triés par horodatage.
pub fn appliquer(medias: &mut [Media], voyage: &Voyage, journees: &[Journee]) -> Bilan {
    let mut bilan = Bilan::default();

    medias.sort_by(|a, b| {
        a.prise_le
            .cmp(&b.prise_le)
            .then_with(|| a.fichier_source.cmp(&b.fichier_source))
    });

    promouvoir_videos(medias, &mut bilan);
    interpoler(medias, &mut bilan);
    heriter(medias, voyage, journees, &mut bilan);

    bilan.sans_position = medias
        .iter()
        .filter(|m| m.fiabilite == Fiabilite::Absente)
        .count();
    bilan
}

/// C1, clause vidéo : une vidéo hérite de la fiabilité de la photo fiable la
/// plus proche dans le temps, sous condition de distance.
fn promouvoir_videos(medias: &mut [Media], bilan: &mut Bilan) {
    // Photos fiables, dans l'ordre chronologique.
    let reperes: Vec<(chrono::DateTime<chrono::FixedOffset>, Position)> = medias
        .iter()
        .filter(|m| m.type_media == TypeMedia::Photo && m.fiabilite == Fiabilite::Haute)
        .filter_map(|m| m.prise_le.zip(m.position))
        .collect();
    if reperes.is_empty() {
        return;
    }

    for media in medias.iter_mut() {
        if media.type_media != TypeMedia::Video || media.fiabilite != Fiabilite::Basse {
            continue;
        }
        let (Some(instant), Some(position)) = (media.prise_le, media.position) else {
            continue;
        };
        // Repère le plus proche dans le temps.
        let plus_proche = reperes.iter().min_by_key(|(t, _)| {
            (*t - instant).num_seconds().abs()
        });
        let Some((instant_repere, position_repere)) = plus_proche else {
            continue;
        };
        let minutes = (*instant_repere - instant).num_minutes().abs();
        if minutes > MINUTES_PROMOTION_VIDEO {
            continue;
        }
        if distance_m(&position, position_repere) > METRES_PROMOTION_VIDEO {
            continue;
        }
        // L'anomalie reste : c'est un fait constaté. Seul le verdict change.
        media.fiabilite = Fiabilite::Haute;
        bilan.videos_promues += 1;
    }
}

/// Interpolation temporelle entre deux positions fiables de la même journée.
fn interpoler(medias: &mut [Media], bilan: &mut Bilan) {
    let bornes: Vec<Option<(NaiveDate, chrono::DateTime<chrono::FixedOffset>, Position)>> = medias
        .iter()
        .map(|m| match (m.jour, m.prise_le, m.position, m.fiabilite) {
            (Some(j), Some(t), Some(p), Fiabilite::Haute) => Some((j, t, p)),
            _ => None,
        })
        .collect();

    for indice in 0..medias.len() {
        if medias[indice].position.is_some() {
            continue;
        }
        let (Some(jour), Some(instant)) = (medias[indice].jour, medias[indice].prise_le) else {
            continue;
        };

        let avant = bornes[..indice]
            .iter()
            .rev()
            .flatten()
            .find(|(j, _, _)| *j == jour);
        let apres = bornes[indice + 1..]
            .iter()
            .flatten()
            .find(|(j, _, _)| *j == jour);
        let (Some((_, t0, p0)), Some((_, t1, p1))) = (avant, apres) else {
            continue;
        };

        if (instant - *t0).num_minutes().abs() > MINUTES_INTERPOLATION
            || (*t1 - instant).num_minutes().abs() > MINUTES_INTERPOLATION
        {
            continue;
        }
        if distance_m(p0, p1) > METRES_INTERPOLATION {
            continue;
        }

        let total = (*t1 - *t0).num_seconds();
        if total <= 0 {
            continue;
        }
        let part = (instant - *t0).num_seconds() as f64 / total as f64;
        let media = &mut medias[indice];
        media.position = Some(Position {
            lat: p0.lat + (p1.lat - p0.lat) * part,
            lon: p0.lon + (p1.lon - p0.lon) * part,
            alt: None,
        });
        media.fiabilite = Fiabilite::Basse;
        media.origine_position = Some(OriginePosition::Interpolee);
        bilan.interpolees += 1;
    }
}

/// D5 : héritage de la position du lieu de la journée.
fn heriter(medias: &mut [Media], voyage: &Voyage, journees: &[Journee], bilan: &mut Bilan) {
    let positions: BTreeMap<&str, &crate::voyage::Lieu> =
        voyage.lieux.iter().map(|l| (l.id.as_str(), l)).collect();
    let par_jour: BTreeMap<NaiveDate, &str> = journees
        .iter()
        .filter_map(|j| j.lieu.as_deref().map(|l| (j.date, l)))
        .collect();

    let mut manquants: Vec<NaiveDate> = Vec::new();
    for media in medias.iter_mut() {
        if media.position.is_some() {
            continue;
        }
        let Some(jour) = media.jour else {
            continue;
        };
        let Some(lieu_id) = par_jour.get(&jour) else {
            if !manquants.contains(&jour) {
                manquants.push(jour);
            }
            continue;
        };
        let Some(lieu) = positions.get(lieu_id) else {
            // Référence inconnue : c'est `carnet check` qui le signalera,
            // le pipeline ne devine pas.
            continue;
        };
        media.position = Some(Position {
            lat: lieu.position.lat,
            lon: lieu.position.lon,
            alt: lieu.position.alt,
        });
        media.fiabilite = Fiabilite::Basse;
        media.origine_position = Some(OriginePosition::Heritee);
        media.lieu = Some(lieu.id.clone());
        bilan.heritees += 1;
    }
    manquants.sort();
    bilan.jours_sans_lieu = manquants;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noms::Convention;
    use crate::scan::{Anomalie, OrigineDate};
    use chrono::DateTime;

    fn media(id: &str, instant: &str, position: Option<(f64, f64)>, fiabilite: Fiabilite) -> Media {
        let prise_le = DateTime::parse_from_rfc3339(instant).ok();
        Media {
            id: id.to_string(),
            type_media: TypeMedia::Photo,
            fichier_source: format!("{id}.jpg"),
            prise_le,
            origine_date: OrigineDate::Exif,
            jour: prise_le.map(|d| d.date_naive()),
            position: position.map(|(lat, lon)| Position {
                lat,
                lon,
                alt: Some(1000.0),
            }),
            fiabilite,
            origine_position: position.map(|_| OriginePosition::Exif),
            lieu: None,
            anomalies: Vec::new(),
            largeur: None,
            hauteur: None,
            orientation: None,
            appareil: None,
            convention: Convention::Telephone,
            octets: 1,
        }
    }

    fn video(id: &str, instant: &str, position: (f64, f64)) -> Media {
        let mut m = media(id, instant, Some(position), Fiabilite::Basse);
        m.type_media = TypeMedia::Video;
        m.fichier_source = format!("{id}.mp4");
        m.anomalies.push(Anomalie::AltitudeNulle);
        m
    }

    #[test]
    fn distance_connue() {
        let a = Position { lat: 45.0, lon: 7.0, alt: None };
        let b = Position { lat: 45.0, lon: 7.01, alt: None };
        let d = distance_m(&a, &b);
        assert!((d - 787.0).abs() < 5.0, "distance mesurée {d}");
    }

    /// C1, clause vidéo : promotion quand le temps et la distance concordent.
    #[test]
    fn video_promue_par_une_photo_proche() {
        let mut medias = vec![
            media("PHOTO", "2026-08-14T10:00:00+02:00", Some((45.5, 7.4)), Fiabilite::Haute),
            video("VIDEO", "2026-08-14T10:05:00+02:00", (45.5005, 7.4005)),
        ];
        let mut bilan = Bilan::default();
        promouvoir_videos(&mut medias, &mut bilan);
        assert_eq!(bilan.videos_promues, 1);
        let v = medias.iter().find(|m| m.id == "VIDEO").expect("vidéo");
        assert_eq!(v.fiabilite, Fiabilite::Haute);
        // L'anomalie constatée reste, seul le verdict change.
        assert!(v.anomalies.contains(&Anomalie::AltitudeNulle));
    }

    /// La condition de distance est ce qui empêche une position gelée d'être
    /// promue par une photo prise au même moment ailleurs.
    #[test]
    fn video_non_promue_si_la_position_est_loin() {
        let mut medias = vec![
            media("PHOTO", "2026-08-14T10:00:00+02:00", Some((45.5, 7.4)), Fiabilite::Haute),
            video("VIDEO", "2026-08-14T10:05:00+02:00", (45.6, 7.6)),
        ];
        let mut bilan = Bilan::default();
        promouvoir_videos(&mut medias, &mut bilan);
        assert_eq!(bilan.videos_promues, 0);
    }

    #[test]
    fn video_non_promue_si_la_photo_est_trop_ancienne() {
        let mut medias = vec![
            media("PHOTO", "2026-08-14T09:00:00+02:00", Some((45.5, 7.4)), Fiabilite::Haute),
            video("VIDEO", "2026-08-14T10:05:00+02:00", (45.5001, 7.4001)),
        ];
        let mut bilan = Bilan::default();
        promouvoir_videos(&mut medias, &mut bilan);
        assert_eq!(bilan.videos_promues, 0);
    }

    #[test]
    fn interpolation_entre_deux_reperes() {
        let mut medias = vec![
            media("A", "2026-08-14T10:00:00+02:00", Some((45.0, 7.0)), Fiabilite::Haute),
            media("B", "2026-08-14T10:10:00+02:00", None, Fiabilite::Absente),
            media("C", "2026-08-14T10:20:00+02:00", Some((45.02, 7.0)), Fiabilite::Haute),
        ];
        let mut bilan = Bilan::default();
        interpoler(&mut medias, &mut bilan);
        assert_eq!(bilan.interpolees, 1);
        let b = medias.iter().find(|m| m.id == "B").expect("média B");
        let position = b.position.expect("position interpolée");
        assert!((position.lat - 45.01).abs() < 1e-6, "latitude {}", position.lat);
        assert_eq!(b.fiabilite, Fiabilite::Basse);
        assert_eq!(b.origine_position, Some(OriginePosition::Interpolee));
        // Une position reconstituée ne prétend pas connaître l'altitude.
        assert!(position.alt.is_none());
    }

    #[test]
    fn pas_d_interpolation_au_dela_des_garde_fous() {
        let mut medias = vec![
            media("A", "2026-08-14T08:00:00+02:00", Some((45.0, 7.0)), Fiabilite::Haute),
            media("B", "2026-08-14T10:00:00+02:00", None, Fiabilite::Absente),
            media("C", "2026-08-14T12:00:00+02:00", Some((45.02, 7.0)), Fiabilite::Haute),
        ];
        let mut bilan = Bilan::default();
        interpoler(&mut medias, &mut bilan);
        assert_eq!(bilan.interpolees, 0);
    }

    #[test]
    fn pas_d_interpolation_entre_deux_journees() {
        let mut medias = vec![
            media("A", "2026-08-14T23:50:00+02:00", Some((45.0, 7.0)), Fiabilite::Haute),
            media("B", "2026-08-15T00:00:00+02:00", None, Fiabilite::Absente),
            media("C", "2026-08-15T00:10:00+02:00", Some((45.02, 7.0)), Fiabilite::Haute),
        ];
        let mut bilan = Bilan::default();
        interpoler(&mut medias, &mut bilan);
        // Les bornes doivent appartenir à la journée du média : ici, une seule.
        assert_eq!(bilan.interpolees, 0);
    }
}
