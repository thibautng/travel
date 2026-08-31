//! Traces : modes de déplacement, et construction des `LineString`.
//!
//! Voir SPEC.md, sections 5.6, 9.2 et D6.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::itineraire::{Itineraires, Resolution};
use crate::jours::Journee;
use crate::lieux::distance_m;
use crate::overrides::Overrides;
use crate::scan::{Fiabilite, Media, OriginePosition, Position};
use crate::voyage::Voyage;

/// Mode de déplacement d'un tronçon de trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Route,
    Marche,
    Velo,
    Bateau,
    Train,
    Telepherique,
}

impl Mode {
    /// Valeur par défaut des forçages d'itinéraire, qui ne concernent que la route.
    pub fn route() -> Self {
        Mode::Route
    }

    /// Couleur de la trace, dérivée du mode et jamais stockée à la main.
    ///
    /// La première palette suivait les neutres chauds de la section 9.5. À
    /// l’usage, six teintes rabattues vers le beige ne se distinguaient plus :
    /// le brun de la route se confondait avec le violet du train et avec le
    /// bleu du bateau, sur un fond Positron déjà gris. Les teintes sont donc
    /// franches et écartées sur la roue, la sobriété revenant à la page qui
    /// entoure la carte. La route, qui pèse les trois quarts des kilomètres,
    /// prend le brun le plus sombre : elle sert de fond aux autres.
    pub fn couleur(self) -> &'static str {
        match self {
            Mode::Route => "#4a3b2e",
            Mode::Marche => "#d1491f",
            Mode::Velo => "#2e7d32",
            Mode::Bateau => "#1565c0",
            Mode::Train => "#8e24aa",
            Mode::Telepherique => "#b08300",
        }
    }

    /// Profil de routage d'OpenRouteService, quand ce mode en a un.
    ///
    /// Chaque mode va sur son réseau : la route sur la chaussée, le vélo sur
    /// les pistes cyclables, la marche sur les sentiers. Le bateau, le train
    /// et le téléphérique n'en ont aucun : c'est la garde de D6.
    pub fn profil(self) -> Option<&'static str> {
        match self {
            Mode::Route => Some("driving-car"),
            Mode::Velo => Some("cycling-regular"),
            Mode::Marche => Some("foot-walking"),
            Mode::Bateau | Mode::Train | Mode::Telepherique => None,
        }
    }

    /// Vrai si ce mode peut partir au moteur d'itinéraire.
    ///
    /// C'est la garde de D6 : un bateau n'a pas d'itinéraire, un train ne
    /// suit pas la route. Un test vérifie qu'aucun mode sans réseau ne passe.
    pub fn calculable(self) -> bool {
        self.profil().is_some()
    }

    /// Nom lisible, pour les rapports en console.
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Route => "route",
            Mode::Marche => "marche",
            Mode::Velo => "vélo",
            Mode::Bateau => "bateau",
            Mode::Train => "train",
            Mode::Telepherique => "téléphérique",
        }
    }

    /// Mode le plus probable pour une vitesse moyenne, en km/h.
    ///
    /// Proposition, jamais un verdict : le résultat est soumis à correction
    /// dans `overrides.yaml`. Les seuils sont volontairement grossiers, une
    /// vitesse moyenne entre deux photos ne distingue pas un train d'une
    /// voiture, et le bateau ressemble au vélo.
    pub fn depuis_vitesse(kmh: f64) -> Mode {
        if kmh < 6.0 {
            Mode::Marche
        } else if kmh < 25.0 {
            Mode::Velo
        } else {
            Mode::Route
        }
    }

    /// La vitesse seule ne suffit pas. Entre deux photos espacées de plusieurs
    /// heures, une voiture qui s’arrête pour déjeuner a la vitesse moyenne d’un
    /// marcheur ; l’inférence rendait alors une marche. Tant que le tronçon
    /// restait une droite, l’erreur était discrète ; routée sur les sentiers,
    /// elle produit une randonnée de trente kilomètres à travers un massif.
    ///
    /// La distance donne un second signal, indépendant du temps écoulé : on ne
    /// marche pas d’une photo à la suivante en s’éloignant de plus de quelques
    /// kilomètres à vol d’oiseau, parce qu’on photographie en chemin. Au-delà
    /// du seuil, le tronçon redevient de la route. Un mode déclaré dans
    /// `overrides.yaml` n’est jamais soumis à cette correction : seul l’humain
    /// sait ce qu’il a fait.
    pub fn corrige_par_distance(self, metres: f64) -> Mode {
        match self {
            Mode::Marche if metres > METRES_MARCHE_MAXIMUM => Mode::Route,
            autre => autre,
        }
    }
}

/// D'où vient le tracé. Détermine le style de rendu (SPEC.md, section 9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceTrace {
    /// Positions EXIF fiables reliées entre elles.
    Mesuree,
    /// Itinéraire produit par le moteur de routage, mode `route` uniquement.
    Calculee,
    /// Points saisis dans `segments` d'`overrides.yaml`.
    Manuelle,
    /// Polyligne des lieux successifs, faute de toute position de média.
    Heritee,
}

impl SourceTrace {
    pub fn nom(self) -> &'static str {
        match self {
            SourceTrace::Mesuree => "mesurée",
            SourceTrace::Calculee => "calculée",
            SourceTrace::Manuelle => "manuelle",
            SourceTrace::Heritee => "héritée",
        }
    }
}

/// Un tronçon de trace : une `Feature` de type `LineString` à l'émission.
#[derive(Debug, Clone)]
pub struct Troncon {
    pub jour: NaiveDate,
    pub mode: Mode,
    pub source: SourceTrace,
    /// Points en ordre GeoJSON, `[longitude, latitude]`.
    pub points: Vec<[f64; 2]>,
}

impl Troncon {
    /// Longueur du tronçon, en kilomètres.
    pub fn longueur_km(&self) -> f64 {
        self.points
            .windows(2)
            .map(|paire| {
                let a = Position {
                    lat: paire[0][1],
                    lon: paire[0][0],
                    alt: None,
                };
                let b = Position {
                    lat: paire[1][1],
                    lon: paire[1][0],
                    alt: None,
                };
                distance_m(&a, &b)
            })
            .sum::<f64>()
            / 1000.0
    }
}

/// Un média positionné, rendu en `Point` sur la carte.
#[derive(Debug, Clone)]
pub struct PointMedia {
    pub id: String,
    pub jour: NaiveDate,
    pub position: Position,
    pub fiabilite: Fiabilite,
    pub origine: Option<OriginePosition>,
}

#[derive(Debug, Default)]
pub struct BilanTraces {
    pub jours_traces: usize,
    /// Journées qui portent des médias mais aucune trace.
    pub jours_sans_trace: Vec<NaiveDate>,
    /// Kilomètres par journée et par mode, tels qu'inférés par la vitesse.
    /// C'est le tableau à relire et à corriger dans `overrides.yaml`.
    pub km_par_jour: BTreeMap<NaiveDate, BTreeMap<Mode, f64>>,
    pub troncons_calcules: usize,
    /// Tronçons routiers restés droits, faute d'itinéraire disponible.
    pub troncons_droits: usize,
    /// Lots à répartir dont la journée ne porte aucun segment manuel.
    pub repartitions_sans_segment: Vec<NaiveDate>,
    /// Journées de déplacement dont l’itinéraire entre camps a été calculé.
    pub transits: usize,
    /// Journées de déplacement dont l’itinéraire n’a pas pu l’être.
    pub transits_manques: Vec<NaiveDate>,
    /// Trajets calculés entre le camp et la première ou la dernière photo.
    pub sorties_de_camp: usize,
}

pub struct Traces {
    pub troncons: Vec<Troncon>,
    pub points: Vec<PointMedia>,
    pub bilan: BilanTraces,
}

/// Distance en deçà de laquelle deux positions consécutives ne méritent pas
/// un tronçon : c'est le bruit GPS d'un appareil immobile.
/// Au-delà de cet écart à vol d’oiseau, deux photos consécutives ne sont pas
/// reliées par une marche. Cinq kilomètres laissent passer les longues boucles
/// de randonnée du 14 août, et écartent les transferts en voiture des journées
/// à plusieurs sites, où l’écart dépasse partout huit kilomètres.
const METRES_MARCHE_MAXIMUM: f64 = 5_000.0;

const METRES_MINIMUM: f64 = 25.0;

/// Construit les traces du voyage.
pub fn construire(
    medias: &[Media],
    voyage: &Voyage,
    journees: &[Journee],
    overrides: &Overrides,
    itineraires: &mut Itineraires,
) -> Traces {
    let mut traces = Traces {
        troncons: Vec::new(),
        points: Vec::new(),
        bilan: BilanTraces::default(),
    };

    // Points de médias : tout ce qui porte une position, quelle qu'en soit
    // l'origine. Le style de rendu, lui, dépendra de la fiabilité.
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

    let mut par_jour: BTreeMap<NaiveDate, Vec<&Media>> = BTreeMap::new();
    for media in medias {
        if media.fiabilite == Fiabilite::Haute && media.position.is_some() {
            if let Some(jour) = media.jour {
                par_jour.entry(jour).or_default().push(media);
            }
        }
    }
    let mut jours_avec_medias: Vec<NaiveDate> = medias.iter().filter_map(|m| m.jour).collect();
    jours_avec_medias.sort();
    jours_avec_medias.dedup();

    for (jour, mut medias_du_jour) in par_jour {
        medias_du_jour.sort_by_key(|m| m.prise_le);
        let troncons = tracer_journee(
            jour,
            &medias_du_jour,
            overrides,
            voyage.fuseau,
            itineraires,
            &mut traces.bilan,
        );
        if !troncons.is_empty() {
            traces.bilan.jours_traces += 1;
        }
        traces.troncons.extend(troncons);
    }

    // Segments saisis à la main : ils forment leurs propres tronçons. Un
    // segment marqué `calculer` fait tracer la route entre ses points par le
    // moteur, ce qui sert aux trajets routiers qu'aucune photo ne documente.
    for segment in &overrides.segments {
        let mut points = segment.points.clone();
        let mut source = SourceTrace::Manuelle;

        if segment.calculer && segment.mode.calculable() && segment.points.len() >= 2 {
            let depart = Position {
                lat: segment.points[0][1],
                lon: segment.points[0][0],
                alt: None,
            };
            let dernier = segment.points[segment.points.len() - 1];
            let arrivee = Position {
                lat: dernier[1],
                lon: dernier[0],
                alt: None,
            };
            let passages = &segment.points[1..segment.points.len() - 1];
            if let Ok(Resolution::Cache(trajet)) | Ok(Resolution::Calcule(trajet)) =
                itineraires.resoudre(segment.mode, &depart, &arrivee, passages)
            {
                points = trajet.points;
                source = SourceTrace::Calculee;
                traces.bilan.troncons_calcules += 1;
            }
        }

        traces.troncons.push(Troncon {
            jour: segment.jour,
            mode: segment.mode,
            source,
            points,
        });
    }

    // Un lot à répartir dont la journée ne porte aucun segment manuel est une
    // consigne inapplicable : elle doit être dite, pas ignorée.
    for lot in overrides.lots.iter().filter(|l| l.repartir_sur_segment) {
        let Some(jour) = lot.jour else { continue };
        if !overrides.segments.iter().any(|s| s.jour == jour) {
            traces.bilan.repartitions_sans_segment.push(jour);
        }
    }

    tracer_sorties_de_camp(medias, voyage, overrides, itineraires, &mut traces);
    tracer_retours(overrides, &mut traces);
    tracer_transits(medias, voyage, overrides, itineraires, &mut traces);
    heriter_des_lieux(voyage, journees, &mut traces);

    let traces_par_jour: BTreeMap<NaiveDate, ()> =
        traces.troncons.iter().map(|t| (t.jour, ())).collect();
    traces.bilan.jours_sans_trace = jours_avec_medias
        .into_iter()
        .filter(|j| !traces_par_jour.contains_key(j))
        .collect();

    for troncon in &traces.troncons {
        *traces
            .bilan
            .km_par_jour
            .entry(troncon.jour)
            .or_default()
            .entry(troncon.mode)
            .or_insert(0.0) += troncon.longueur_km();
    }

    traces
}

/// Découpe la journée en tronçons homogènes, le mode étant inféré de la
/// vitesse entre deux positions consécutives.
fn tracer_journee(
    jour: NaiveDate,
    medias: &[&Media],
    overrides: &Overrides,
    fuseau: chrono_tz::Tz,
    itineraires: &mut Itineraires,
    bilan: &mut BilanTraces,
) -> Vec<Troncon> {
    let mut troncons: Vec<Troncon> = Vec::new();
    let mut courant: Option<Troncon> = None;

    for paire in medias.windows(2) {
        let (Some(p0), Some(p1)) = (paire[0].position, paire[1].position) else {
            continue;
        };
        let (Some(t0), Some(t1)) = (paire[0].prise_le, paire[1].prise_le) else {
            continue;
        };
        let metres = distance_m(&p0, &p1);
        if metres < METRES_MINIMUM {
            continue;
        }
        // Un mode déclaré dans overrides.yaml l'emporte sur l'inférence : la
        // vitesse moyenne d'une voiture arrêtée pour déjeuner est celle d'un
        // vélo, et seul l'humain sait laquelle des deux c'était.
        let heures = (t1 - t0).num_seconds() as f64 / 3600.0;
        // L'heure est ramenée au fuseau du voyage avant d'interroger les
        // forçages : les vidéos portent leur horodatage en temps universel,
        // et une plage écrite en heure locale les manquerait de deux heures.
        // Une tranche déclarée sans trace ne produit rien : son trajet est écrit
        // à la main dans `segments`, et une droite de plus le doublerait.
        if overrides.sans_trace(jour, t0.with_timezone(&fuseau).time()) {
            if let Some(termine) = courant.take() {
                troncons.push(termine);
            }
            continue;
        }

        let mode = overrides
            .mode_force(jour, t0.with_timezone(&fuseau).time())
            .unwrap_or(if heures <= 0.0 {
                Mode::Route
            } else {
                Mode::depuis_vitesse(metres / 1000.0 / heures).corrige_par_distance(metres)
            });

        // Un tronçon routier part au moteur d'itinéraire et forme sa propre
        // Feature : sa source diffère de celle des tronçons mesurés.
        if mode.calculable() {
            if let Some(termine) = courant.take() {
                troncons.push(termine);
            }
            // Les points de passage se filtrent par mode : un col imposé à la
            // voiture n'a rien à faire dans un itinéraire cyclable.
            let passages: Vec<[f64; 2]> = overrides
                .itineraires
                .iter()
                .filter(|f| f.jour == jour && f.mode == mode)
                .flat_map(|f| f.points_de_passage.clone())
                .collect();
            match itineraires.resoudre(mode, &p0, &p1, &passages) {
                Ok(Resolution::Cache(trajet)) | Ok(Resolution::Calcule(trajet)) => {
                    bilan.troncons_calcules += 1;
                    troncons.push(Troncon {
                        jour,
                        mode,
                        source: SourceTrace::Calculee,
                        points: trajet.points,
                    });
                }
                _ => {
                    bilan.troncons_droits += 1;
                    troncons.push(Troncon {
                        jour,
                        mode,
                        source: SourceTrace::Mesuree,
                        points: vec![[p0.lon, p0.lat], [p1.lon, p1.lat]],
                    });
                }
            }
            continue;
        }

        match courant.as_mut() {
            Some(t) if t.mode == mode => t.points.push([p1.lon, p1.lat]),
            _ => {
                if let Some(termine) = courant.take() {
                    troncons.push(termine);
                }
                courant = Some(Troncon {
                    jour,
                    mode,
                    source: SourceTrace::Mesuree,
                    points: vec![[p0.lon, p0.lat], [p1.lon, p1.lat]],
                });
            }
        }
    }
    if let Some(termine) = courant {
        troncons.push(termine);
    }
    troncons
}

/// Camp où l'on a dormi la nuit qui suit `nuit`.
///
/// Un camp déclaré `du` au `au` couvre les nuits de `du` inclus à `au` exclu :
/// arriver le 12 et repartir le 15, c'est y dormir les 12, 13 et 14.
fn camp_de_la_nuit(voyage: &Voyage, nuit: NaiveDate) -> Option<&crate::voyage::Lieu> {
    voyage.lieux.iter().find(|lieu| {
        lieu.type_lieu == crate::voyage::TypeLieu::Camp
            && lieu.du.map(|d| d <= nuit).unwrap_or(false)
            && lieu.au.map(|a| nuit < a).unwrap_or(false)
    })
}

/// Distance en deçà de laquelle un déplacement entre deux camps ne mérite pas
/// d'itinéraire : on n'a pas vraiment bougé.
const METRES_TRANSIT_MINIMUM: f64 = 3_000.0;

/// Trace les journées de déplacement d'un camp à l'autre.
///
/// Les photos ne documentent pas les trajets : sur 4 400 km annoncés, elles
/// n'en dessinent que 888, et les jours de transit les plus longs sont ceux
/// où l'on photographie le moins. Quand le camp du soir diffère de celui de
/// la veille, l'itinéraire routier de l'un à l'autre est donc calculé.
///
/// La trace produite est `heritee` : elle ne dit pas par où l'on est passé,
/// elle dit d'où l'on est parti et où l'on est arrivé, la route entre les
/// deux étant celle que propose le moteur. `points_de_passage` dans
/// `overrides.yaml` corrige un trajet plausible mais faux.
fn tracer_transits(
    medias: &[Media],
    voyage: &Voyage,
    overrides: &Overrides,
    itineraires: &mut Itineraires,
    traces: &mut Traces,
) {
    let mut jour = voyage.date_debut;
    while jour <= voyage.date_fin {
        let veille = jour.pred_opt().and_then(|d| camp_de_la_nuit(voyage, d));
        let soir = camp_de_la_nuit(voyage, jour);

        // Même camp au coucher qu'au réveil : la journée est sur place.
        if let (Some(a), Some(b)) = (veille, soir) {
            if a.id == b.id {
                jour = match jour.succ_opt() {
                    Some(suivant) => suivant,
                    None => break,
                };
                continue;
            }
        }

        // Au premier et au dernier jour, un des deux bouts manque : c'est le
        // domicile qui en tient lieu, à défaut la position connue la plus
        // extrême de la journée.
        let domicile = |reference: &Option<String>| -> Option<Position> {
            reference.as_deref().and_then(|id| {
                voyage
                    .lieux
                    .iter()
                    .find(|l| l.id == id)
                    .map(position_du_lieu)
            })
        };
        let depart = veille.map(position_du_lieu).or_else(|| {
            if jour == voyage.date_debut {
                domicile(&voyage.depart)
            } else {
                None
            }
            .or_else(|| extremite_du_jour(medias, jour, true))
        });
        let arrivee = soir.map(position_du_lieu).or_else(|| {
            if jour == voyage.date_fin {
                domicile(&voyage.arrivee)
            } else {
                None
            }
            .or_else(|| extremite_du_jour(medias, jour, false))
        });

        let (Some(depart), Some(arrivee)) = (depart, arrivee) else {
            jour = match jour.succ_opt() {
                Some(suivant) => suivant,
                None => break,
            };
            continue;
        };

        // Les points de passage du jour s'appliquent aussi au transit : c'est
        // ainsi que l'on impose les nationales plutôt que l'autoroute, ou un
        // col plutôt qu'un tunnel.
        let passages: Vec<[f64; 2]> = overrides
            .itineraires
            .iter()
            .filter(|f| f.jour == jour && f.mode.calculable())
            .flat_map(|f| f.points_de_passage.clone())
            .collect();

        if distance_m(&depart, &arrivee) >= METRES_TRANSIT_MINIMUM {
            if let Ok(Resolution::Cache(trajet)) | Ok(Resolution::Calcule(trajet)) =
                itineraires.resoudre(Mode::Route, &depart, &arrivee, &passages)
            {
                // L'itinéraire du jour remplace les tronçons routiers déduits
                // de la vitesse : c'est le même trajet, tracé en mieux.
                traces
                    .troncons
                    .retain(|t| !(t.jour == jour && t.mode == Mode::Route));
                traces.troncons.push(Troncon {
                    jour,
                    mode: Mode::Route,
                    source: SourceTrace::Heritee,
                    points: trajet.points,
                });
                traces.bilan.transits += 1;
            } else {
                traces.bilan.transits_manques.push(jour);
            }
        }

        jour = match jour.succ_opt() {
            Some(suivant) => suivant,
            None => break,
        };
    }
}

fn position_du_lieu(lieu: &crate::voyage::Lieu) -> Position {
    Position {
        lat: lieu.position.lat,
        lon: lieu.position.lon,
        alt: lieu.position.alt,
    }
}

/// Première ou dernière position connue d'une journée, quelle que soit sa
/// fiabilité : pour amorcer un transit, une position approximative vaut mieux
/// que pas de trace du tout.
fn extremite_du_jour(medias: &[Media], jour: NaiveDate, premiere: bool) -> Option<Position> {
    media_extreme(medias, jour, premiere).and_then(|m| m.position)
}

fn media_extreme(medias: &[Media], jour: NaiveDate, premiere: bool) -> Option<&Media> {
    let mut du_jour: Vec<&Media> = medias
        .iter()
        .filter(|m| m.jour == Some(jour) && m.position.is_some() && m.prise_le.is_some())
        .collect();
    du_jour.sort_by_key(|m| m.prise_le);
    if premiere {
        du_jour.first().copied()
    } else {
        du_jour.last().copied()
    }
}

/// Distance à partir de laquelle le trajet camp-photo mérite d'être tracé.
///
/// On sort d'un camping à pied sans que cela fasse une étape ; au-delà d'un
/// kilomètre, on a pris la voiture, et le trajet manque à la trace.
const METRES_SORTIE_DE_CAMP: f64 = 1_000.0;

/// Relie le camp à la première photo du jour, et la dernière au camp.
///
/// Les journées sur place n'étaient tracées qu'entre leur première et leur
/// dernière photo : le trajet depuis le camping n'existait nulle part, alors
/// qu'on y a dormi. Le 4 août, la journée se réduisait à 1,1 km de voiture
/// entre deux photos de la Nadiža, sans le trajet qui y mène.
///
/// Le raisonnement est celui du camp : on part de là où l'on a dormi et l'on y
/// revient. Les journées de déplacement, elles, changent de camp et relèvent
/// de `tracer_transits`.
fn tracer_sorties_de_camp(
    medias: &[Media],
    voyage: &Voyage,
    overrides: &Overrides,
    itineraires: &mut Itineraires,
    traces: &mut Traces,
) {
    let mut jour = voyage.date_debut;
    while jour <= voyage.date_fin {
        let suivant = jour.succ_opt();
        let veille = jour.pred_opt().and_then(|d| camp_de_la_nuit(voyage, d));
        let soir = camp_de_la_nuit(voyage, jour);

        // Seulement les journées sur place : un changement de camp est un
        // transit, tracé ailleurs et d'un camp à l'autre.
        let sur_place = match (veille, soir) {
            (Some(a), Some(b)) if a.id == b.id => Some(position_du_lieu(a)),
            _ => None,
        };

        if let Some(camp) = sur_place {
            for (repere, depuis_le_camp) in [
                (media_extreme(medias, jour, true), true),
                (media_extreme(medias, jour, false), false),
            ] {
                let Some(media) = repere else { continue };
                let Some(photo) = media.position else { continue };
                // Une tranche déclarée sans trace vaut aussi pour la sortie du
                // camp : le 10 août, la montée à l’alpe s’est faite en
                // téléphérique, et router une voiture jusqu’à la première photo
                // emprunterait une route interdite aux voitures.
                if let Some(instant) = media.prise_le {
                    if overrides.sans_trace(jour, instant.with_timezone(&voyage.fuseau).time()) {
                        continue;
                    }
                }
                if distance_m(&camp, &photo) < METRES_SORTIE_DE_CAMP {
                    continue;
                }
                let (depart, arrivee) = if depuis_le_camp {
                    (camp, photo)
                } else {
                    (photo, camp)
                };
                // Sans itinéraire, on n'écrit rien. Ce tronçon est une
                // déduction, pas une mesure : si le moteur ne sait pas relier le
                // camp à la photo, une droite de dix kilomètres à travers la
                // montagne affirmerait plus que ce que l'on sait. Le 31 juillet,
                // la dernière photo est prise sur le bateau, au milieu du lac.
                let points = match itineraires.resoudre(Mode::Route, &depart, &arrivee, &[]) {
                    Ok(Resolution::Cache(trajet)) | Ok(Resolution::Calcule(trajet)) => {
                        traces.bilan.sorties_de_camp += 1;
                        trajet.points
                    }
                    _ => continue,
                };
                traces.troncons.push(Troncon {
                    jour,
                    mode: Mode::Route,
                    source: SourceTrace::Heritee,
                    points,
                });
            }
        }

        jour = match suivant {
            Some(j) => j,
            None => break,
        };
    }
}

/// Ajoute le retour des journées déclarées en aller-retour.
///
/// Le retour est l'aller à l'envers : on recolle les tronçons du jour dans ce
/// mode, dans l'ordre où ils ont été bâtis, et on les parcourt à rebours. La
/// source est `manuelle` : c'est une déduction de l'humain, pas une mesure.
fn tracer_retours(overrides: &Overrides, traces: &mut Traces) {
    for retour in &overrides.retours {
        let aller: Vec<[f64; 2]> = traces
            .troncons
            .iter()
            .filter(|t| t.jour == retour.jour && t.mode == retour.mode)
            .flat_map(|t| t.points.iter().copied())
            .collect();
        if aller.len() < 2 {
            continue;
        }
        let mut points = aller;
        points.reverse();
        traces.troncons.push(Troncon {
            jour: retour.jour,
            mode: retour.mode,
            source: SourceTrace::Manuelle,
            points,
        });
    }
}

/// Trace de repli pour les journées sans aucune position : la polyligne des
/// lieux successifs. Voir D5 et le lot 7.
fn heriter_des_lieux(voyage: &Voyage, journees: &[Journee], traces: &mut Traces) {
    if journees.is_empty() {
        return;
    }
    let positions: BTreeMap<&str, &crate::voyage::Lieu> =
        voyage.lieux.iter().map(|l| (l.id.as_str(), l)).collect();
    let deja_trace: BTreeMap<NaiveDate, ()> =
        traces.troncons.iter().map(|t| (t.jour, ())).collect();

    let mut precedent: Option<[f64; 2]> = None;
    for journee in journees {
        let point = journee
            .lieu
            .as_deref()
            .and_then(|id| positions.get(id))
            .map(|lieu| [lieu.position.lon, lieu.position.lat]);
        let Some(point) = point else {
            continue;
        };
        if let Some(avant) = precedent {
            if !deja_trace.contains_key(&journee.date) && avant != point {
                traces.troncons.push(Troncon {
                    jour: journee.date,
                    mode: Mode::Route,
                    source: SourceTrace::Heritee,
                    points: vec![avant, point],
                });
            }
        }
        precedent = Some(point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noms::Convention;
    use crate::scan::{OrigineDate, TypeMedia};
    use chrono::DateTime;

    /// D6 amendé : chaque mode terrestre a son réseau, les autres n'en ont
    /// aucun. Un bateau n'a pas d'itinéraire, un train ne suit pas la route.
    #[test]
    fn seuls_les_modes_terrestres_partent_au_calcul() {
        assert_eq!(Mode::Route.profil(), Some("driving-car"));
        assert_eq!(Mode::Velo.profil(), Some("cycling-regular"));
        assert_eq!(Mode::Marche.profil(), Some("foot-walking"));
        for mode in [Mode::Bateau, Mode::Train, Mode::Telepherique] {
            assert!(
                !mode.calculable(),
                "{} ne doit jamais être calculé",
                mode.nom()
            );
        }
    }

    #[test]
    fn couleur_de_la_marche_conforme_a_la_spec() {
        assert_eq!(Mode::Marche.couleur(), "#d1491f");
    }

    #[test]
    fn inference_par_la_vitesse() {
        assert_eq!(Mode::depuis_vitesse(3.5), Mode::Marche);
        assert_eq!(Mode::depuis_vitesse(15.0), Mode::Velo);
        assert_eq!(Mode::depuis_vitesse(70.0), Mode::Route);
    }

    /// Le piège du 3 août : neuf kilomètres à vol d’oiseau entre deux photos
    /// espacées de quatre heures, soit 2 km/h. La vitesse dit la marche, la
    /// distance dit la voiture, et c’était le col du Vršič.
    #[test]
    fn la_distance_corrige_les_marches_trop_longues() {
        assert_eq!(Mode::Marche.corrige_par_distance(9_400.0), Mode::Route);
        assert_eq!(Mode::Marche.corrige_par_distance(3_800.0), Mode::Marche);
        // Le vélo et la route ne sont pas concernés : quarante kilomètres à
        // vélo dans la journée du 6 août sont parfaitement ordinaires.
        assert_eq!(Mode::Velo.corrige_par_distance(40_000.0), Mode::Velo);
    }

    #[test]
    fn lecture_depuis_le_yaml() {
        let m: Mode = serde_norway::from_str("bateau").expect("mode lisible");
        assert_eq!(m, Mode::Bateau);
    }

    fn media(id: &str, instant: &str, lat: f64, lon: f64) -> Media {
        let prise_le = DateTime::parse_from_rfc3339(instant).ok();
        Media {
            id: id.to_string(),
            type_media: TypeMedia::Photo,
            fichier_source: format!("{id}.jpg"),
            prise_le,
            origine_date: OrigineDate::Exif,
            jour: prise_le.map(|d| d.date_naive()),
            position: Some(Position {
                lat,
                lon,
                alt: Some(1200.0),
            }),
            fiabilite: Fiabilite::Haute,
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

    fn itineraires_vides() -> Itineraires {
        // Sans clé ni cache : les tronçons routiers restent droits, ce qui
        // suffit à exercer la construction.
        Itineraires::charger(Path::new("dossier-inexistant"), "aucun").expect("cache vide")
    }

    use std::path::Path;

    #[test]
    fn une_marche_donne_un_troncon_mesure() {
        // Environ 800 mètres en dix minutes, soit 4,7 km/h.
        let medias = [
            media("A", "2026-08-14T10:00:00+02:00", 45.5, 7.4),
            media("B", "2026-08-14T10:10:00+02:00", 45.5, 7.41),
        ];
        let refs: Vec<&Media> = medias.iter().collect();
        let mut bilan = BilanTraces::default();
        let troncons = tracer_journee(
            NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            &refs,
            &Overrides::default(),
            chrono_tz::Europe::Paris,
            &mut itineraires_vides(),
            &mut bilan,
        );
        assert_eq!(troncons.len(), 1);
        assert_eq!(troncons[0].mode, Mode::Marche);
        assert_eq!(troncons[0].source, SourceTrace::Mesuree);
    }

    #[test]
    fn le_bruit_gps_ne_cree_pas_de_troncon() {
        // Dix mètres : un appareil immobile.
        let medias = [
            media("A", "2026-08-14T10:00:00+02:00", 45.5, 7.4),
            media("B", "2026-08-14T10:05:00+02:00", 45.50008, 7.4),
        ];
        let refs: Vec<&Media> = medias.iter().collect();
        let mut bilan = BilanTraces::default();
        let troncons = tracer_journee(
            NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            &refs,
            &Overrides::default(),
            chrono_tz::Europe::Paris,
            &mut itineraires_vides(),
            &mut bilan,
        );
        assert!(troncons.is_empty());
    }

    #[test]
    fn un_troncon_routier_sans_itineraire_reste_droit() {
        // Vingt kilomètres en dix minutes : 120 km/h.
        let medias = [
            media("A", "2026-08-14T10:00:00+02:00", 45.5, 7.4),
            media("B", "2026-08-14T10:10:00+02:00", 45.68, 7.4),
        ];
        let refs: Vec<&Media> = medias.iter().collect();
        let mut bilan = BilanTraces::default();
        let troncons = tracer_journee(
            NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            &refs,
            &Overrides::default(),
            chrono_tz::Europe::Paris,
            &mut itineraires_vides(),
            &mut bilan,
        );
        assert_eq!(troncons.len(), 1);
        assert_eq!(troncons[0].mode, Mode::Route);
        assert_eq!(troncons[0].source, SourceTrace::Mesuree);
        assert_eq!(bilan.troncons_droits, 1);
        assert_eq!(bilan.troncons_calcules, 0);
    }

    /// Une voiture arrêtée pour déjeuner affiche la vitesse moyenne d'un
    /// vélo. Le mode déclaré doit l'emporter sur l'inférence.
    #[test]
    fn le_mode_declare_emporte_sur_l_inference() {
        // Huit cents mètres en dix minutes : l'inférence dirait « marche ».
        let medias = [
            media("A", "2026-08-14T10:00:00+02:00", 45.5, 7.4),
            media("B", "2026-08-14T10:10:00+02:00", 45.5, 7.41),
        ];
        let refs: Vec<&Media> = medias.iter().collect();
        let overrides: Overrides = serde_norway::from_str(
            "modes:\n  - jour: 2026-08-14\n    mode: bateau\n",
        )
        .expect("yaml lisible");
        let mut bilan = BilanTraces::default();
        let troncons = tracer_journee(
            NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            &refs,
            &overrides,
            chrono_tz::Europe::Paris,
            &mut itineraires_vides(),
            &mut bilan,
        );
        assert_eq!(troncons.len(), 1);
        assert_eq!(troncons[0].mode, Mode::Bateau);
    }

    #[test]
    fn longueur_d_un_troncon() {
        let troncon = Troncon {
            jour: NaiveDate::from_ymd_opt(2026, 8, 14).unwrap(),
            mode: Mode::Marche,
            source: SourceTrace::Mesuree,
            points: vec![[7.0, 45.0], [7.01, 45.0]],
        };
        assert!((troncon.longueur_km() - 0.787).abs() < 0.01);
    }
}
