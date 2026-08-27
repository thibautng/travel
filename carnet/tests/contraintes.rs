//! Un test par contrainte de SPEC.md, section 8.
//!
//! Les fixtures sont des JPEG minimaux fabriqués par `tests/fixtures/generer.py`,
//! qui documente pour chaque cas le fichier réel dont il est tiré.

use carnet::quality::{self, Bilan};
use carnet::scan::{self, Anomalie, Fiabilite, Media, OrigineDate};
use carnet::voyage::Voyage;
use chrono::NaiveDate;
use std::path::PathBuf;

fn jour(a: i32, m: u32, j: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, j).expect("date de test valide")
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn voyage_de_test(dossiers_ignores: Vec<String>) -> Voyage {
    Voyage {
        id: "fixtures".to_string(),
        titre: "Fixtures des contraintes".to_string(),
        sous_titre: None,
        date_debut: jour(2026, 7, 24),
        date_fin: jour(2026, 8, 15),
        pays: vec![],
        distance_km: None,
        nuits: None,
        mode: None,
        fuseau: chrono_tz::Europe::Paris,
        source_photos: fixtures(),
        dossiers_ignores,
        notion: None,
        lieux: vec![],
    }
}

/// Inventorie les fixtures comme le ferait `carnet scan`.
fn inventorier() -> (Vec<Media>, Bilan) {
    let voyage = voyage_de_test(vec!["[Originals]".to_string()]);
    let mut inventaire =
        scan::inventorier(&voyage, true).expect("les fixtures doivent s'inventorier sans erreur");
    let bilan = quality::evaluer(&mut inventaire.medias, &voyage);
    (inventaire.medias, bilan)
}

fn media<'a>(medias: &'a [Media], id: &str) -> &'a Media {
    medias
        .iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("fixture « {id} » absente de l'inventaire"))
}

/// C1 : c'est l'altitude non nulle qui distingue une position satellite d'une
/// position réseau, pas la présence du champ.
#[test]
fn c1_altitude_nulle_declasse_la_position() {
    let (medias, _) = inventorier();

    let fiable = media(&medias, "c01_altitude_reelle");
    assert_eq!(fiable.fiabilite, Fiabilite::Haute);
    assert!(!fiable.anomalies.contains(&Anomalie::AltitudeNulle));

    let suspecte = media(&medias, "c01_altitude_nulle");
    assert_eq!(suspecte.fiabilite, Fiabilite::Basse);
    assert!(suspecte.anomalies.contains(&Anomalie::AltitudeNulle));
    // La position reste dans media.json, elle n'est pas effacée.
    assert!(suspecte.position.is_some());
}

/// C2 : deux médias à la même position, à plus de vingt minutes.
#[test]
fn c2_positions_clonees_signalees() {
    let (medias, bilan) = inventorier();

    for id in ["c02_clone_matin", "c02_clone_soir"] {
        assert!(
            media(&medias, id).anomalies.contains(&Anomalie::PositionClonee),
            "{id} devrait porter l'anomalie position_clonee"
        );
    }
    assert!(bilan.clones_groupes >= 1);

    // Deux médias pris au même instant au même endroit ne sont pas des clones :
    // c'est un doublon légitime, pas une position gelée.
    assert!(!media(&medias, "c01_altitude_reelle")
        .anomalies
        .contains(&Anomalie::PositionClonee));
}

/// C3 : le nom porte la date du partage, l'EXIF porte la vraie date.
#[test]
fn c3_le_nom_ne_prime_jamais_sur_l_exif() {
    let (medias, _) = inventorier();
    let repartage = media(&medias, "IMG_20260730_071148");

    assert_eq!(repartage.origine_date, OrigineDate::Exif);
    assert_eq!(repartage.jour, Some(jour(2026, 7, 28)));
    assert!(repartage.anomalies.contains(&Anomalie::NomMenteur));
}

/// C4 : horloge perdue, déduite des bornes du voyage.
#[test]
fn c4_horloge_perdue_detectee() {
    let (medias, _) = inventorier();
    let gopro = media(&medias, "GOPR2699");

    assert!(gopro.anomalies.contains(&Anomalie::HorlogePerdue));
    assert_eq!(gopro.jour, Some(jour(2016, 1, 3)));
    assert_eq!(gopro.fiabilite, Fiabilite::Absente);
    assert!(gopro.position.is_none());
}

/// C5 : le pipeline ne sait pas où la trace aurait dû passer, il signale
/// seulement un saut anormal.
#[test]
fn c5_trou_de_trace_signale_comme_candidat() {
    let (_, bilan) = inventorier();
    let trous: Vec<_> = bilan
        .trous
        .iter()
        .filter(|t| t.jour == jour(2026, 8, 6))
        .collect();

    assert!(
        !trous.is_empty(),
        "le saut du 6 août devrait être signalé comme candidat"
    );
    assert!(trous.iter().any(|t| t.motif.contains("saut de")));
}

/// C6 : le tilde est normalisé, la variante conservée, l'anomalie posée.
#[test]
fn c6_nom_hostile_normalise() {
    let (medias, _) = inventorier();
    let variante = media(&medias, "IMG20260808113008-2");

    assert!(variante.anomalies.contains(&Anomalie::NomNormalise));
    assert_eq!(variante.fichier_source, "IMG20260808113008~2.jpg");
}

/// C7 : la volumétrie rapportée correspond au dossier réellement parcouru.
#[test]
fn c7_volumetrie_du_dossier() {
    let (medias, _) = inventorier();
    // Douze médias à la racine. Le treizième est dans [Originals], ignoré,
    // et generer.py n'est pas un média.
    assert_eq!(medias.len(), 12);
    assert!(medias.iter().all(|m| m.octets > 0));
}

/// C8 : le sous-dossier d'originaux est ignoré par défaut.
#[test]
fn c8_sous_dossier_ignore() {
    let voyage = voyage_de_test(vec!["[Originals]".to_string()]);
    let inventaire = scan::inventorier(&voyage, true).expect("inventaire possible");

    assert_eq!(inventaire.dossiers_sautes.len(), 1);
    assert!(inventaire.medias.iter().filter(|m| m.id == "c08_homonyme").count() == 1);
}

/// C8 : sans cette exclusion, la collision fait échouer la commande. Aucun
/// arbitrage automatique, jamais.
#[test]
fn c8_collision_fait_echouer_le_scan() {
    let voyage = voyage_de_test(vec![]);
    let erreur = scan::inventorier(&voyage, true)
        .expect_err("deux homonymes doivent faire échouer l'inventaire");

    let message = erreur.to_string();
    assert!(message.contains("collision"), "message obtenu : {message}");
    assert!(message.contains("c08_homonyme"), "message obtenu : {message}");
}

/// C9 : sans les références d'hémisphère, la Polynésie atterrit dans le
/// Pacifique nord, à 4 000 km de sa position.
#[test]
fn c9_hemisphere_sud_et_ouest_appliques() {
    let (medias, _) = inventorier();
    let position = media(&medias, "c09_hemisphere_sud_ouest")
        .position
        .expect("position présente");

    assert!(position.lat < 0.0, "latitude sud attendue : {}", position.lat);
    assert!(position.lon < 0.0, "longitude ouest attendue : {}", position.lon);
    assert!((position.lat + 17.539_24).abs() < 0.001);
    assert!((position.lon + 149.567_65).abs() < 0.001);
}

/// C10 : aucun EXIF, la date vient du nom, et seulement dans ce cas.
#[test]
fn c10_fichier_sans_exif_date_par_son_nom() {
    let (medias, _) = inventorier();
    let messagerie = media(&medias, "IMG-20260811-WA0000");

    assert!(messagerie.anomalies.contains(&Anomalie::ExifAbsent));
    assert!(messagerie.anomalies.contains(&Anomalie::DateDuNom));
    assert_eq!(messagerie.origine_date, OrigineDate::Nom);
    assert_eq!(messagerie.jour, Some(jour(2026, 8, 11)));
    assert_eq!(messagerie.fiabilite, Fiabilite::Absente);
}

/// La datation par le nom est un dernier recours : un fichier qui a un EXIF
/// ne doit jamais l'emprunter à son nom.
#[test]
fn la_date_du_nom_ne_sert_qu_en_dernier_recours() {
    let (medias, _) = inventorier();
    let par_le_nom = medias
        .iter()
        .filter(|m| m.origine_date == OrigineDate::Nom)
        .count();

    assert_eq!(par_le_nom, 1, "seule la fixture C10 doit être datée par son nom");
}
