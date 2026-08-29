//! Écriture de `data/<voyage>/media.json` et rapport de console.
//!
//! Voir SPEC.md, section 6.2, étape 13, et section 11, lot 1.

use crate::noms::Convention;
use crate::quality::Bilan;
use crate::scan::{Anomalie, Fiabilite, Inventaire, Media, OrigineDate, TypeMedia};
use crate::voyage::Voyage;
use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ErreurEmission {
    #[error("création de {chemin} impossible")]
    Dossier {
        chemin: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("écriture de {chemin} impossible")]
    Ecriture {
        chemin: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("sérialisation de media.json impossible")]
    Serialisation(#[from] serde_json::Error),
}

/// Écrit `data/<voyage>/media.json`.
pub fn ecrire_media_json(
    depot: &Path,
    voyage: &Voyage,
    medias: &[Media],
) -> Result<PathBuf, ErreurEmission> {
    let dossier = depot.join("data").join(&voyage.id);
    std::fs::create_dir_all(&dossier).map_err(|source| ErreurEmission::Dossier {
        chemin: dossier.clone(),
        source,
    })?;
    let chemin = dossier.join("media.json");
    let texte = serde_json::to_string_pretty(medias)?;
    std::fs::write(&chemin, texte).map_err(|source| ErreurEmission::Ecriture {
        chemin: chemin.clone(),
        source,
    })?;
    Ok(chemin)
}

/// Go et Mo au sens décimal, comme les annonce SPEC.md, section 8, C7.
fn octets_lisibles(octets: u64) -> String {
    let go = octets as f64 / 1_000_000_000.0;
    if go >= 1.0 {
        return format!("{go:.1} Go").replace('.', ",");
    }
    format!("{:.0} Mo", octets as f64 / 1_000_000.0)
}

fn pourcent(partie: usize, total: usize) -> String {
    if total == 0 {
        return "0 %".to_string();
    }
    format!("{:.0} %", 100.0 * partie as f64 / total as f64)
}

fn nom_convention(convention: Convention) -> &'static str {
    match convention {
        Convention::Telephone => "IMG/VID + horodatage (téléphone)",
        Convention::Repartage => "IMG_date_heure (repartagé, C3)",
        Convention::Messagerie => "IMG-date-WAnnnn (messagerie, C10)",
        Convention::GoPro => "GOPRnnnn (GoPro, C4)",
        Convention::Compact => "Pnnnnnnn (compact)",
        Convention::AppareilPhoto => "DSCnnnn (appareil photo)",
        Convention::Autre => "hors convention",
    }
}

fn nom_anomalie(anomalie: Anomalie) -> &'static str {
    match anomalie {
        Anomalie::AltitudeNulle => "altitude_nulle (C1)",
        Anomalie::PositionClonee => "position_clonee (C2)",
        Anomalie::NomMenteur => "nom_menteur (C3)",
        Anomalie::HorlogePerdue => "horloge_perdue (C4)",
        Anomalie::Homonyme => "homonyme (C8)",
        Anomalie::NomNormalise => "nom_normalise (C6)",
        Anomalie::ExifAbsent => "exif_absent (C10)",
        Anomalie::DateDuNom => "date_du_nom (C10)",
        Anomalie::HemisphereAbsent => "hemisphere_absent (C9)",
    }
}

/// Rapport de `carnet stats`. Une section par famille de contraintes.
pub fn rapport(voyage: &Voyage, inventaire: &Inventaire, bilan: &Bilan) {
    let medias = &inventaire.medias;
    let total = medias.len();
    let photos = medias
        .iter()
        .filter(|m| m.type_media == TypeMedia::Photo)
        .count();
    let videos = total - photos;
    let octets: u64 = medias.iter().map(|m| m.octets).sum();
    let octets_video: u64 = medias
        .iter()
        .filter(|m| m.type_media == TypeMedia::Video)
        .map(|m| m.octets)
        .sum();

    println!();
    println!("VOYAGE  {} ({})", voyage.titre, voyage.id);
    println!("  source        {}", voyage.source_photos.display());
    println!("  période       {} au {}", voyage.date_debut, voyage.date_fin);
    println!("  fuseau        {}", voyage.fuseau);

    println!();
    println!("VOLUMÉTRIE (C7)");
    println!(
        "  {total} médias      {photos} photos, {videos} vidéos, {} dont {} de vidéo",
        octets_lisibles(octets),
        octets_lisibles(octets_video)
    );
    if inventaire.non_medias > 0 {
        println!("  {} fichiers non médias écartés", inventaire.non_medias);
    }
    if !inventaire.dossiers_sautes.is_empty() {
        println!();
        println!("DOSSIERS IGNORÉS (C8)");
        for dossier in &inventaire.dossiers_sautes {
            println!("  {}", dossier.display());
        }
    }

    // Datation.
    let par_exif = medias
        .iter()
        .filter(|m| m.origine_date == OrigineDate::Exif)
        .count();
    let par_nom = medias
        .iter()
        .filter(|m| m.origine_date == OrigineDate::Nom)
        .count();
    let sans_date = medias
        .iter()
        .filter(|m| m.origine_date == OrigineDate::Absente)
        .count();
    println!();
    println!("DATATION (C3, C4, C10)");
    println!("  par l'EXIF    {par_exif} ({})", pourcent(par_exif, total));
    println!("  par le nom    {par_nom}");
    println!("  sans date     {sans_date}");

    // Positions.
    let haute = medias
        .iter()
        .filter(|m| m.fiabilite == Fiabilite::Haute)
        .count();
    let basse = medias
        .iter()
        .filter(|m| m.fiabilite == Fiabilite::Basse)
        .count();
    let absente = medias
        .iter()
        .filter(|m| m.fiabilite == Fiabilite::Absente)
        .count();
    let geolocalises = haute + basse;
    println!();
    println!("POSITIONS (C1, C2, C9)");
    println!(
        "  géolocalisés  {geolocalises} sur {total} ({})",
        pourcent(geolocalises, total)
    );
    println!(
        "  fiables       {haute} ({} des géolocalisés)",
        pourcent(haute, geolocalises)
    );
    println!(
        "  suspectes     {basse} ({} des géolocalisés)",
        pourcent(basse, geolocalises)
    );
    println!("  sans position {absente}");
    println!(
        "  clones        {} groupes, {} médias, dont {} groupes à altitude réelle identique",
        bilan.clones_groupes, bilan.clones_medias, bilan.clones_triplet_identique
    );

    // Conventions de nommage.
    let mut conventions: BTreeMap<&str, usize> = BTreeMap::new();
    for media in medias {
        *conventions.entry(nom_convention(media.convention)).or_default() += 1;
    }
    println!();
    println!("NOMS (C6)");
    for (convention, nombre) in &conventions {
        println!("  {nombre:>5}  {convention}");
    }

    // Anomalies.
    let mut anomalies: BTreeMap<&str, usize> = BTreeMap::new();
    for media in medias {
        for anomalie in &media.anomalies {
            *anomalies.entry(nom_anomalie(*anomalie)).or_default() += 1;
        }
    }
    println!();
    println!("ANOMALIES");
    if anomalies.is_empty() {
        println!("  aucune");
    }
    for (anomalie, nombre) in &anomalies {
        println!("  {nombre:>5}  {anomalie}");
    }

    // Journées.
    let mut par_jour: BTreeMap<NaiveDate, usize> = BTreeMap::new();
    for media in medias {
        if let Some(jour) = media.jour {
            *par_jour.entry(jour).or_default() += 1;
        }
    }
    println!();
    println!("JOURNÉES  {} jours couverts", par_jour.len());
    let hors_periode: Vec<_> = par_jour
        .keys()
        .filter(|j| **j < voyage.date_debut || **j > voyage.date_fin)
        .collect();
    if !hors_periode.is_empty() {
        println!(
            "  hors période  {}",
            hors_periode
                .iter()
                .map(|j| j.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    println!();
    println!("TROUS DE TRACE CANDIDATS (C5)  {}", bilan.trous.len());
    for trou in &bilan.trous {
        println!("  {}  {}", trou.jour, trou.motif);
    }
    println!();
}

// ---------------------------------------------------------------------------
// Traces et journées agrégées, lot 2
// ---------------------------------------------------------------------------

use crate::itineraire::Itineraires;
use crate::jours::Journee;
use crate::lieux::Bilan as BilanLieux;
use crate::overrides::Journal;
use crate::track::{Mode, Traces};
use chrono::{DateTime, FixedOffset};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Comptes {
    pub photo: usize,
    pub video: usize,
    pub total: usize,
}

/// Index dérivé d'une journée. Ne contient jamais de contenu rédactionnel :
/// voir SPEC.md, section 4, seconde règle de frontière.
#[derive(Debug, Serialize)]
pub struct JourAgrege {
    pub jour: NaiveDate,
    pub lieu: Option<String>,
    pub camp: Option<String>,
    pub premiere_prise: Option<DateTime<FixedOffset>>,
    pub derniere_prise: Option<DateTime<FixedOffset>>,
    pub medias: Comptes,
    pub couverture: Option<String>,
    /// `[lon_min, lat_min, lon_max, lat_max]`.
    pub bbox: Option<[f64; 4]>,
    pub distance_trace_km: f64,
    pub modes: Vec<Mode>,
    pub anomalies: Vec<String>,
}

/// Construit l'index des journées à partir des médias et des traces.
pub fn construire_jours(
    medias: &[Media],
    journees: &[Journee],
    traces: &Traces,
    bilan: &Bilan,
) -> Vec<JourAgrege> {
    let frontmatter: BTreeMap<NaiveDate, &Journee> =
        journees.iter().map(|j| (j.date, j)).collect();

    let mut jours: BTreeMap<NaiveDate, Vec<&Media>> = BTreeMap::new();
    for media in medias {
        if let Some(jour) = media.jour {
            jours.entry(jour).or_default().push(media);
        }
    }

    jours
        .into_iter()
        .map(|(jour, medias_du_jour)| {
            let photo = medias_du_jour
                .iter()
                .filter(|m| m.type_media == TypeMedia::Photo)
                .count();
            let total = medias_du_jour.len();

            let instants: Vec<DateTime<FixedOffset>> =
                medias_du_jour.iter().filter_map(|m| m.prise_le).collect();

            let mut bbox: Option<[f64; 4]> = None;
            for media in &medias_du_jour {
                let Some(position) = media.position else {
                    continue;
                };
                bbox = Some(match bbox {
                    None => [position.lon, position.lat, position.lon, position.lat],
                    Some(b) => [
                        b[0].min(position.lon),
                        b[1].min(position.lat),
                        b[2].max(position.lon),
                        b[3].max(position.lat),
                    ],
                });
            }

            let troncons: Vec<_> = traces.troncons.iter().filter(|t| t.jour == jour).collect();
            let mut modes: Vec<Mode> = troncons.iter().map(|t| t.mode).collect();
            modes.sort();
            modes.dedup();

            // La couverture déclarée par la rédaction l'emporte ; à défaut,
            // le premier média fiable de la journée.
            let couverture = frontmatter
                .get(&jour)
                .and_then(|j| j.couverture.clone())
                .or_else(|| {
                    medias_du_jour
                        .iter()
                        .find(|m| {
                            m.type_media == TypeMedia::Photo && m.fiabilite == Fiabilite::Haute
                        })
                        .map(|m| m.id.clone())
                });

            let anomalies = bilan
                .trous
                .iter()
                .filter(|t| t.jour == jour)
                .map(|_| "trou_candidat".to_string())
                .collect();

            JourAgrege {
                jour,
                lieu: frontmatter.get(&jour).and_then(|j| j.lieu.clone()),
                camp: frontmatter.get(&jour).and_then(|j| j.camp.clone()),
                premiere_prise: instants.iter().min().copied(),
                derniere_prise: instants.iter().max().copied(),
                medias: Comptes {
                    photo,
                    video: total - photo,
                    total,
                },
                couverture,
                bbox,
                distance_trace_km: (troncons.iter().map(|t| t.longueur_km()).sum::<f64>() * 10.0)
                    .round()
                    / 10.0,
                modes,
                anomalies,
            }
        })
        .collect()
}

fn ecrire(dossier: &Path, nom: &str, texte: &str) -> Result<PathBuf, ErreurEmission> {
    std::fs::create_dir_all(dossier).map_err(|source| ErreurEmission::Dossier {
        chemin: dossier.to_path_buf(),
        source,
    })?;
    let chemin = dossier.join(nom);
    std::fs::write(&chemin, texte).map_err(|source| ErreurEmission::Ecriture {
        chemin: chemin.clone(),
        source,
    })?;
    Ok(chemin)
}

/// Écrit `data/<voyage>/jours.json`.
pub fn ecrire_jours_json(
    depot: &Path,
    voyage: &Voyage,
    jours: &[JourAgrege],
) -> Result<PathBuf, ErreurEmission> {
    let dossier = depot.join("data").join(&voyage.id);
    ecrire(&dossier, "jours.json", &serde_json::to_string_pretty(jours)?)
}

/// Écrit `data/<voyage>/trace.geojson`.
///
/// Une `Feature` de type `LineString` par tronçon, une `Feature` de type
/// `Point` par média positionné. Voir SPEC.md, section 5.6.
pub fn ecrire_trace_geojson(
    depot: &Path,
    voyage: &Voyage,
    traces: &Traces,
) -> Result<PathBuf, ErreurEmission> {
    let mut features: Vec<geojson::Feature> = Vec::new();

    for troncon in &traces.troncons {
        let mut proprietes = geojson::JsonObject::new();
        proprietes.insert("jour".to_string(), troncon.jour.to_string().into());
        proprietes.insert("mode".to_string(), nom_mode_json(troncon.mode).into());
        proprietes.insert("source".to_string(), nom_source_json(troncon.source).into());
        proprietes.insert("couleur".to_string(), troncon.mode.couleur().into());
        features.push(geojson::Feature {
            bbox: None,
            geometry: Some(geojson::Geometry::new(geojson::Value::LineString(
                troncon.points.iter().map(|p| vec![p[0], p[1]]).collect(),
            ))),
            id: None,
            properties: Some(proprietes),
            foreign_members: None,
        });
    }

    for point in &traces.points {
        let mut proprietes = geojson::JsonObject::new();
        proprietes.insert("id".to_string(), point.id.clone().into());
        proprietes.insert("jour".to_string(), point.jour.to_string().into());
        proprietes.insert(
            "fiabilite".to_string(),
            match point.fiabilite {
                Fiabilite::Haute => "haute",
                Fiabilite::Basse => "basse",
                Fiabilite::Absente => "absente",
            }
            .into(),
        );
        if let Some(origine) = point.origine {
            proprietes.insert(
                "origine_position".to_string(),
                serde_json::to_value(origine)
                    .unwrap_or(serde_json::Value::Null),
            );
        }
        features.push(geojson::Feature {
            bbox: None,
            geometry: Some(geojson::Geometry::new(geojson::Value::Point(vec![
                point.position.lon,
                point.position.lat,
            ]))),
            id: None,
            properties: Some(proprietes),
            foreign_members: None,
        });
    }

    let collection = geojson::FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };
    let dossier = depot.join("data").join(&voyage.id);
    ecrire(&dossier, "trace.geojson", &collection.to_string())
}

fn nom_mode_json(mode: Mode) -> &'static str {
    match mode {
        Mode::Route => "route",
        Mode::Marche => "marche",
        Mode::Velo => "velo",
        Mode::Bateau => "bateau",
        Mode::Train => "train",
        Mode::Telepherique => "telepherique",
    }
}

fn nom_source_json(source: crate::track::SourceTrace) -> &'static str {
    use crate::track::SourceTrace;
    match source {
        SourceTrace::Mesuree => "mesuree",
        SourceTrace::Calculee => "calculee",
        SourceTrace::Manuelle => "manuelle",
        SourceTrace::Heritee => "heritee",
    }
}

/// Rapport des traces, commun à `carnet build` et `carnet check`.
pub fn rapport_traces(
    traces: &Traces,
    journal: &Journal,
    lieux: &BilanLieux,
    itineraires: &Itineraires,
) {
    println!();
    println!("SURCHARGES (section 7)");
    println!("  appliquées    {}", journal.total_applique());
    for (motif, nombre) in &journal.lots_appliques {
        println!("    lot {motif} : {nombre} fichiers");
    }
    if !journal.exclus.is_empty() {
        println!("    exclusions : {}", journal.exclus.len());
    }
    if journal.inutilisees.is_empty() {
        println!("  inutilisées   aucune");
    } else {
        println!("  INUTILISÉES   {}", journal.inutilisees.len());
        for motif in &journal.inutilisees {
            println!("    {motif}");
        }
    }

    println!();
    println!("POSITIONS RECONSTITUÉES (D5, C1)");
    println!("  vidéos promues {}", lieux.videos_promues);
    println!("  interpolées    {}", lieux.interpolees);
    println!("  héritées       {}", lieux.heritees);
    println!("  sans position  {}", lieux.sans_position);
    if !lieux.jours_sans_lieu.is_empty() {
        println!(
            "  {} journées sans lieu déclaré, dont des médias auraient pu hériter",
            lieux.jours_sans_lieu.len()
        );
    }

    println!();
    println!("ITINÉRAIRES (D6)");
    println!(
        "  clé {}, cache {} entrées, {} appels réseau",
        if itineraires.cle_presente() {
            "présente"
        } else {
            "ABSENTE"
        },
        itineraires.taille_cache(),
        itineraires.appels
    );
    println!(
        "  tronçons routiers : {} calculés, {} restés droits",
        traces.bilan.troncons_calcules, traces.bilan.troncons_droits
    );

    println!();
    println!("TRACES  {} tronçons", traces.troncons.len());
    let mut par_source: BTreeMap<&str, usize> = BTreeMap::new();
    for troncon in &traces.troncons {
        *par_source.entry(troncon.source.nom()).or_default() += 1;
    }
    for (source, nombre) in &par_source {
        println!("  {nombre:>5}  {source}");
    }
    if !traces.bilan.jours_sans_trace.is_empty() {
        println!(
            "  {} journées sans trace : {}",
            traces.bilan.jours_sans_trace.len(),
            traces
                .bilan
                .jours_sans_trace
                .iter()
                .map(|j| j.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for jour in &traces.bilan.repartitions_sans_segment {
        println!("  ATTENTION  répartition demandée le {jour}, aucun segment manuel ce jour-là");
    }

    println!();
    println!("MODES INFÉRÉS PAR LA VITESSE, à relire et corriger dans overrides.yaml");
    for (jour, modes) in &traces.bilan.km_par_jour {
        let detail: Vec<String> = modes
            .iter()
            .map(|(mode, km)| format!("{} {:.1} km", mode.nom(), km))
            .collect();
        println!("  {jour}  {}", detail.join(", "));
    }
    println!();
}
