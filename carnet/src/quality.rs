//! Fiabilité des positions, anomalies, trous de trace candidats.
//!
//! Voir SPEC.md, section 6.2, étape 3, et section 8, contraintes C1, C2, C4 et C5.

use crate::scan::{Anomalie, Fiabilite, Media};
use crate::voyage::Voyage;
use chrono::{Duration, NaiveDate};
use std::collections::BTreeMap;

/// Deux positions identiques à la cinquième décimale, soit environ un mètre.
const DECIMALES: f64 = 100_000.0;

/// Au-delà de cet écart, deux médias à la même position sont suspects (C2).
const ECART_CLONE_MINUTES: i64 = 20;

/// Un saut de trace au-delà de ces deux seuils est un trou candidat (C5).
const TROU_KM: f64 = 2.0;
const TROU_MINUTES: i64 = 45;

#[derive(Debug, Clone)]
pub struct TrouCandidat {
    pub jour: NaiveDate,
    pub motif: String,
}

#[derive(Debug, Default)]
pub struct Bilan {
    /// Groupes de médias partageant exactement la même position (C2).
    pub clones_groupes: usize,
    pub clones_medias: usize,
    /// Sous-ensemble des groupes dont l'altitude est elle aussi identique,
    /// et non nulle. Mesure demandée par C2 avant de décider s'il faut les
    /// déclasser.
    pub clones_triplet_identique: usize,
    pub trous: Vec<TrouCandidat>,
}

/// Distance en kilomètres entre deux points, formule de haversine.
///
/// Six lignes plutôt qu'une dépendance à `geo`, qui n'a pas d'autre usage
/// au lot 1 (SPEC.md, section 12).
fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const RAYON: f64 = 6371.0;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * RAYON * a.sqrt().asin()
}

fn ajouter(media: &mut Media, anomalie: Anomalie) {
    if !media.anomalies.contains(&anomalie) {
        media.anomalies.push(anomalie);
    }
}

/// Attribue fiabilité et anomalies, puis calcule le bilan du voyage.
pub fn evaluer(medias: &mut [Media], voyage: &Voyage) -> Bilan {
    let mut bilan = Bilan::default();

    // C1 : l'altitude non nulle est ce qui distingue une position satellite
    // d'une position réseau. Le champ GPSAltitude, lui, est toujours présent.
    for media in medias.iter_mut() {
        media.fiabilite = match media.position {
            None => Fiabilite::Absente,
            Some(position) => match position.alt {
                Some(alt) if alt.abs() > f64::EPSILON => Fiabilite::Haute,
                _ => {
                    ajouter(media, Anomalie::AltitudeNulle);
                    Fiabilite::Basse
                }
            },
        };

        // C4 : une date hors des bornes du voyage trahit une horloge perdue.
        // La règle est générale, elle ne cible pas les GoPro par leur nom.
        if let Some(jour) = media.jour {
            if jour < voyage.date_debut - Duration::days(1)
                || jour > voyage.date_fin + Duration::days(1)
            {
                ajouter(media, Anomalie::HorlogePerdue);
            }
        }
    }

    // C2 : positions clonées, quelle que soit l'altitude.
    let mut groupes: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    for (indice, media) in medias.iter().enumerate() {
        if let Some(position) = media.position {
            let cle = (
                (position.lat * DECIMALES).round() as i64,
                (position.lon * DECIMALES).round() as i64,
            );
            groupes.entry(cle).or_default().push(indice);
        }
    }

    let ecart_max = Duration::minutes(ECART_CLONE_MINUTES);
    let mut a_marquer: Vec<usize> = Vec::new();
    for indices in groupes.values() {
        if indices.len() < 2 {
            continue;
        }
        let mut dates: Vec<_> = indices
            .iter()
            .filter_map(|i| medias[*i].prise_le)
            .collect();
        if dates.len() < 2 {
            continue;
        }
        dates.sort();
        let etendue = match (dates.first(), dates.last()) {
            (Some(debut), Some(fin)) => *fin - *debut,
            _ => continue,
        };
        if etendue <= ecart_max {
            continue;
        }
        bilan.clones_groupes += 1;
        bilan.clones_medias += indices.len();

        let altitudes: Vec<i64> = indices
            .iter()
            .filter_map(|i| medias[*i].position.and_then(|p| p.alt))
            .map(|alt| (alt * 1000.0).round() as i64)
            .collect();
        if altitudes.len() == indices.len()
            && altitudes.iter().all(|a| *a == altitudes[0])
            && altitudes[0] != 0
        {
            bilan.clones_triplet_identique += 1;
        }
        a_marquer.extend(indices.iter().copied());
    }
    for indice in a_marquer {
        ajouter(&mut medias[indice], Anomalie::PositionClonee);
    }

    bilan.trous = trous_candidats(medias);
    bilan
}

/// C5 : signale les journées où la trace saute, sans prétendre savoir où elle
/// aurait dû passer. L'humain tranche dans overrides.yaml.
fn trous_candidats(medias: &[Media]) -> Vec<TrouCandidat> {
    let mut par_jour: BTreeMap<NaiveDate, Vec<&Media>> = BTreeMap::new();
    for media in medias {
        if let Some(jour) = media.jour {
            par_jour.entry(jour).or_default().push(media);
        }
    }

    let mut trous = Vec::new();
    for (jour, medias_du_jour) in par_jour {
        let mut fiables: Vec<(chrono::DateTime<chrono::FixedOffset>, f64, f64)> = medias_du_jour
            .iter()
            .filter(|m| m.fiabilite == Fiabilite::Haute)
            .filter_map(|m| match (m.prise_le, m.position) {
                (Some(date), Some(position)) => Some((date, position.lat, position.lon)),
                _ => None,
            })
            .collect();
        fiables.sort_by_key(|(date, _, _)| *date);

        for paire in fiables.windows(2) {
            let [(date1, lat1, lon1), (date2, lat2, lon2)] = paire else {
                continue;
            };
            let minutes = (*date2 - *date1).num_minutes();
            let km = distance_km(*lat1, *lon1, *lat2, *lon2);
            if km > TROU_KM && minutes > TROU_MINUTES {
                trous.push(TrouCandidat {
                    jour,
                    motif: format!(
                        "saut de {km:.1} km en {minutes} min, entre {} et {}",
                        date1.format("%H:%M"),
                        date2.format("%H:%M")
                    ),
                });
            }
        }

        // Couverture : les positions fiables couvrent-elles la journée ?
        let mut toutes: Vec<_> = medias_du_jour.iter().filter_map(|m| m.prise_le).collect();
        toutes.sort();
        if let (Some(debut), Some(fin), Some(premier), Some(dernier)) = (
            toutes.first(),
            toutes.last(),
            fiables.first().map(|(d, _, _)| *d),
            fiables.last().map(|(d, _, _)| *d),
        ) {
            let amplitude = (*fin - *debut).num_minutes();
            let couverte = (dernier - premier).num_minutes();
            if amplitude > 120 && couverte * 2 < amplitude {
                trous.push(TrouCandidat {
                    jour,
                    motif: format!(
                        "positions fiables sur {couverte} min pour {amplitude} min de photos"
                    ),
                });
            }
        }
    }
    trous
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_connue() {
        // Paris, Notre-Dame vers la tour Eiffel : environ 4,2 km.
        let d = distance_km(48.8530, 2.3499, 48.8584, 2.2945);
        assert!((d - 4.1).abs() < 0.3, "distance calculée : {d}");
    }

    #[test]
    fn distance_nulle() {
        assert!(distance_km(45.5, 7.4, 45.5, 7.4) < 0.000_1);
    }
}
