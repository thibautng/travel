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
