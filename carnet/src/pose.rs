//! Repose sur la trace les positions que l'on ne croit pas.
//!
//! Une position `basse` est, le plus souvent, un relevé réseau : le téléphone
//! n'a pas vu le ciel et rend la position d'une antenne, la même pour trente
//! photos d'affilée. Elle tombe parfois à cinq cents mètres du lieu réel, et
//! au bord d'un lac elle tombe dans l'eau.
//!
//! L'interpolation de `lieux.rs` ne les corrige pas : elle ne s'occupe que
//! des médias *sans* position, et une position clonée en est une, fausse mais
//! présente. Ces médias gardaient donc leur relevé d'antenne, et la carte ne
//! les affichait pas, faute de savoir où les mettre.
//!
//! Une fois la trace construite, on sait où le voyageur se trouvait : entre
//! deux positions fiables, il suivait le tronçon qui les relie, désormais
//! routé sur le réseau du mode. Un média pris dans cet intervalle se pose donc
//! sur cette polyligne, à la fraction que dit son horodatage. Autour de
//! l'Eibsee, les photos reviennent sur le sentier de rive.
//!
//! La position reste `basse` : posée n'est pas mesurée. Son origine devient
//! `posee`, et la carte la rend en pastille creuse (SPEC.md, section 9.2).

use chrono::{DateTime, FixedOffset, NaiveDate};
use std::collections::BTreeMap;

use crate::lieux::distance_m;
use crate::scan::{Fiabilite, Media, OriginePosition, Position};
use crate::track::{PointMedia, Traces, Troncon};

/// Écart maximal, en minutes, entre le média posé et chacune de ses bornes.
///
/// Trois fois la demi-heure que s'accorde l'interpolation de `lieux.rs`, et ce
/// n'est pas une inconstance : l'interpolation tire une droite dans un
/// intervalle dont elle ignore tout, et une heure de droite traverse un
/// massif. Ici le chemin est connu, c'est le tronçon ; seul l'endroit où l'on
/// s'y trouvait est incertain. Se tromper de place le long d'un sentier de
/// rive vaut mieux que rester au milieu du lac, et le tour de l'Eibsee tient
/// une heure dix entre deux photos fiables.
const MINUTES_POSE: i64 = 90;

/// Tolérance, en mètres, pour reconnaître le tronçon qui relie deux bornes.
/// Les tronçons calculés partent de la position exacte de la borne ; le
/// moteur d'itinéraire, lui, ramène parfois le premier point sur la chaussée.
const METRES_RACCORD: f64 = 60.0;

#[derive(Debug, Default)]
pub struct Bilan {
    /// Médias reposés sur un tronçon.
    pub posees: usize,
    /// Médias candidats qu'aucun tronçon ne portait.
    pub sans_troncon: usize,
}

/// Repose les positions `basse` sur la trace, puis renvoie le bilan.
pub fn appliquer(medias: &mut [Media], traces: &Traces) -> Bilan {
    let mut bilan = Bilan::default();

    // Les bornes : positions fiables, dans l'ordre où `lieux::appliquer` a
    // trié les médias, c'est-à-dire chronologique.
    let bornes: Vec<Option<(NaiveDate, DateTime<FixedOffset>, Position)>> = medias
        .iter()
        .map(|m| match (m.jour, m.prise_le, m.position, m.fiabilite) {
            (Some(j), Some(t), Some(p), Fiabilite::Haute) => Some((j, t, p)),
            _ => None,
        })
        .collect();

    let mut par_jour: BTreeMap<NaiveDate, Vec<&Troncon>> = BTreeMap::new();
    for troncon in &traces.troncons {
        if troncon.points.len() >= 2 {
            par_jour.entry(troncon.jour).or_default().push(troncon);
        }
    }

    for indice in 0..medias.len() {
        if medias[indice].fiabilite != Fiabilite::Basse {
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
        if (instant - *t0).num_minutes().abs() > MINUTES_POSE
            || (*t1 - instant).num_minutes().abs() > MINUTES_POSE
        {
            continue;
        }
        let total = (*t1 - *t0).num_seconds();
        if total <= 0 {
            continue;
        }

        let Some(troncon) = par_jour
            .get(&jour)
            .and_then(|liste| liste.iter().find(|t| relie(t, p0, p1)))
        else {
            bilan.sans_troncon += 1;
            continue;
        };

        let part = (instant - *t0).num_seconds() as f64 / total as f64;
        let Some(position) = le_long_de(&troncon.points, part) else {
            bilan.sans_troncon += 1;
            continue;
        };

        let media = &mut medias[indice];
        media.position = Some(position);
        media.origine_position = Some(OriginePosition::Posee);
        bilan.posees += 1;
    }

    bilan
}

/// Vrai si ce tronçon va bien d'une borne à l'autre.
fn relie(troncon: &Troncon, depart: &Position, arrivee: &Position) -> bool {
    let premier = coin(troncon.points[0]);
    let dernier = coin(troncon.points[troncon.points.len() - 1]);
    distance_m(&premier, depart) <= METRES_RACCORD && distance_m(&dernier, arrivee) <= METRES_RACCORD
}

fn coin(point: [f64; 2]) -> Position {
    Position {
        lat: point[1],
        lon: point[0],
        alt: None,
    }
}

/// Point situé à la fraction `part` de la longueur d'une polyligne.
///
/// La fraction est celle du temps, reportée sur la distance : elle suppose
/// une allure constante entre deux bornes, ce qui est faux dans le détail et
/// suffisant pour poser une pastille sur le bon versant d'un lac.
fn le_long_de(points: &[[f64; 2]], part: f64) -> Option<Position> {
    if points.len() < 2 {
        return None;
    }
    let segments: Vec<f64> = points
        .windows(2)
        .map(|paire| distance_m(&coin(paire[0]), &coin(paire[1])))
        .collect();
    let total: f64 = segments.iter().sum();
    if total <= f64::EPSILON {
        return Some(coin(points[0]));
    }

    let vise = total * part.clamp(0.0, 1.0);
    let mut parcouru = 0.0;
    for (indice, longueur) in segments.iter().enumerate() {
        if parcouru + longueur >= vise {
            let reste = if *longueur > f64::EPSILON {
                (vise - parcouru) / longueur
            } else {
                0.0
            };
            let (a, b) = (points[indice], points[indice + 1]);
            return Some(Position {
                lat: a[1] + (b[1] - a[1]) * reste,
                lon: a[0] + (b[0] - a[0]) * reste,
                alt: None,
            });
        }
        parcouru += longueur;
    }
    Some(coin(points[points.len() - 1]))
}

/// Reconstruit les points de la trace après la pose.
///
/// Les tronçons, eux, ne bougent pas : ils sont bâtis sur les seules positions
/// fiables, qu'aucune pose ne touche.
pub fn rafraichir_points(traces: &mut Traces, medias: &[Media]) {
    traces.points.clear();
    for media in medias {
        if let (Some(jour), Some(position)) = (media.jour, media.position) {
            traces.points.push(PointMedia {
                id: media.id.clone(),
                jour,
                position,
                fiabilite: media.fiabilite,
                origine: media.origine_position,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noms::Convention;
    use crate::scan::{OrigineDate, TypeMedia};
    use crate::track::{Mode, SourceTrace};
    use chrono::NaiveDate;

    fn media(id: &str, instant: &str, position: (f64, f64), fiabilite: Fiabilite) -> Media {
        let prise_le = DateTime::parse_from_rfc3339(instant).unwrap();
        Media {
            id: id.to_string(),
            type_media: TypeMedia::Photo,
            fichier_source: format!("{id}.jpg"),
            prise_le: Some(prise_le),
            origine_date: OrigineDate::Exif,
            jour: Some(prise_le.date_naive()),
            position: Some(Position {
                lat: position.0,
                lon: position.1,
                alt: None,
            }),
            fiabilite,
            origine_position: Some(OriginePosition::Exif),
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

    /// Le cas de l'Eibsee, réduit à trois médias : deux bornes sur la rive
    /// est, et au milieu une photo dont le relevé réseau tombe dans le lac.
    /// Le tronçon, lui, contourne le lac par le nord.
    #[test]
    fn la_photo_tombee_dans_l_eau_revient_sur_le_sentier() {
        let jour = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let mut medias = vec![
            media("A", "2026-07-27T16:46:00+02:00", (47.4542, 10.9826), Fiabilite::Haute),
            media("LAC", "2026-07-27T17:05:00+02:00", (47.4575, 10.9804), Fiabilite::Basse),
            media("B", "2026-07-27T17:56:00+02:00", (47.4565, 10.9940), Fiabilite::Haute),
        ];
        let traces = Traces {
            troncons: vec![Troncon {
                jour,
                mode: Mode::Marche,
                source: SourceTrace::Calculee,
                // Rive nord : le sentier passe au-dessus du lac.
                points: vec![
                    [10.9826, 47.4542],
                    [10.9800, 47.4620],
                    [10.9940, 47.4565],
                ],
            }],
            points: Vec::new(),
            bilan: Default::default(),
        };

        let bilan = appliquer(&mut medias, &traces);
        assert_eq!(bilan.posees, 1);
        let pose = medias[1].position.unwrap();
        assert_eq!(medias[1].origine_position, Some(OriginePosition::Posee));
        // La photo n'est plus au milieu du lac, elle est sur la rive nord.
        assert!(pose.lat > 47.459, "posée à {}", pose.lat);
        // Et la fiabilité ne monte pas : posée n'est pas mesurée.
        assert_eq!(medias[1].fiabilite, Fiabilite::Basse);
    }

    /// Sans tronçon reliant les deux bornes, on ne pose rien : mieux vaut un
    /// relevé douteux qu'une position inventée.
    #[test]
    fn sans_troncon_la_position_ne_bouge_pas() {
        let mut medias = vec![
            media("A", "2026-07-27T16:46:00+02:00", (47.4542, 10.9826), Fiabilite::Haute),
            media("LAC", "2026-07-27T17:05:00+02:00", (47.4575, 10.9804), Fiabilite::Basse),
            media("B", "2026-07-27T17:56:00+02:00", (47.4565, 10.9940), Fiabilite::Haute),
        ];
        let traces = Traces {
            troncons: Vec::new(),
            points: Vec::new(),
            bilan: Default::default(),
        };
        let bilan = appliquer(&mut medias, &traces);
        assert_eq!(bilan.posees, 0);
        assert_eq!(bilan.sans_troncon, 1);
        assert_eq!(medias[1].position.unwrap().lat, 47.4575);
    }

    /// Une borne trop lointaine dans le temps ne dit plus rien du parcours.
    #[test]
    fn la_borne_trop_lointaine_ne_pose_pas() {
        let mut medias = vec![
            media("A", "2026-07-27T10:00:00+02:00", (47.4542, 10.9826), Fiabilite::Haute),
            media("LAC", "2026-07-27T17:05:00+02:00", (47.4575, 10.9804), Fiabilite::Basse),
            media("B", "2026-07-27T17:56:00+02:00", (47.4565, 10.9940), Fiabilite::Haute),
        ];
        let traces = Traces {
            troncons: Vec::new(),
            points: Vec::new(),
            bilan: Default::default(),
        };
        assert_eq!(appliquer(&mut medias, &traces).posees, 0);
    }

    #[test]
    fn le_long_d_une_polyligne() {
        let points = vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let milieu = le_long_de(&points, 0.5).unwrap();
        assert!((milieu.lat - 1.0).abs() < 0.05, "lat {}", milieu.lat);
        let fin = le_long_de(&points, 1.0).unwrap();
        assert!((fin.lon - 1.0).abs() < 0.001);
    }
}
