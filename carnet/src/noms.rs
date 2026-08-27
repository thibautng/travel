//! Normalisation des noms de fichiers, identifiants de médias, collisions.
//!
//! Voir SPEC.md, sections 5.3 et 8, contraintes C3, C6, C8 et C10.
//!
//! L'analyse est faite à la main plutôt qu'avec `regex` : les six conventions
//! rencontrées sont des motifs de position fixe, et cela évite une dépendance
//! sans motif (SPEC.md, section 12).

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Conventions de nommage rencontrées dans les quatre dossiers sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convention {
    /// `IMG20260814151616` ou `VID20260725164207`, OPPO Reno6 Pro 5G.
    Telephone,
    /// `IMG_20260730_071148`, fichier repartagé. Le nom ment, voir C3.
    Repartage,
    /// `IMG-20250722-WA0000`, reçu par messagerie, aucun EXIF, voir C10.
    Messagerie,
    /// `GOPR2699`, GoPro HERO7, horloge perdue, voir C4.
    GoPro,
    /// `P1000123`, Panasonic DMC-LX7 de la Polynésie.
    Compact,
    /// `DSC_1234`, `DSCN0001`, autres appareils photo.
    AppareilPhoto,
    Autre,
}

impl Convention {
    /// Vrai si le nom porte une date que l'on peut lire.
    ///
    /// Attention : porter une date ne veut pas dire qu'elle est juste. C3
    /// établit que les noms `Repartage` portent la date du partage, pas celle
    /// de la prise de vue. Cette date ne sert qu'en dernier recours, quand
    /// aucun EXIF n'est lisible (C10).
    pub fn porte_une_date(self) -> bool {
        matches!(
            self,
            Convention::Telephone | Convention::Repartage | Convention::Messagerie
        )
    }
}

/// Résultat de l'analyse d'un nom de fichier.
///
/// `variante` n'est pas encore lu : il sert au lot 6, pour choisir entre une
/// photo et sa version retouchée au moment d'écrire le récit (C6).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NomAnalyse {
    /// Identifiant du média : nom sans extension, normalisé.
    pub identifiant: String,
    pub convention: Convention,
    /// Suffixe de variante conservé, `~2` ou `_01`. Voir C6.
    pub variante: Option<String>,
    /// Horodatage lisible dans le nom, quand il y en a un.
    pub horodatage: Option<NaiveDateTime>,
    /// Vrai si l'identifiant diffère du nom d'origine.
    pub normalise: bool,
}

/// Extensions traitées comme des médias.
pub fn est_media(chemin: &Path) -> bool {
    let Some(ext) = chemin.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "jpg" | "jpeg" | "png" | "heic" | "heif" | "mp4" | "mov" | "3gp"
    )
}

/// Vrai si le média est une vidéo.
pub fn est_video(chemin: &Path) -> bool {
    let Some(ext) = chemin.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(ext.to_ascii_lowercase().as_str(), "mp4" | "mov" | "3gp")
}

/// Remplace tout caractère hors `[A-Za-z0-9_-]` par un tiret.
///
/// `IMG20260808113008~2` devient `IMG20260808113008-2`, qui n'entre en
/// collision avec rien : la variante est conservée, pas effacée.
fn normaliser(base: &str) -> String {
    base.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn chiffres(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn nombre(s: &str) -> Option<u32> {
    if chiffres(s) {
        s.parse().ok()
    } else {
        None
    }
}

/// Construit un horodatage à partir de composantes, ou rien si la date est
/// impossible. Une date invalide n'est pas une erreur : c'est un nom qui ne
/// porte pas de date.
fn horodatage(a: u32, m: u32, j: u32, h: u32, mi: u32, s: u32) -> Option<NaiveDateTime> {
    let date = NaiveDate::from_ymd_opt(a as i32, m, j)?;
    let heure = NaiveTime::from_hms_opt(h, mi, s)?;
    Some(date.and_time(heure))
}

/// Découpe un suffixe de variante en fin de nom : `~2`, `-2`, `_01`.
fn separer_variante(reste: &str) -> Option<String> {
    let mut c = reste.chars();
    let premier = c.next()?;
    if !matches!(premier, '~' | '_' | '-') {
        return None;
    }
    let suite: String = c.collect();
    if chiffres(&suite) {
        Some(suite)
    } else {
        None
    }
}

/// Lit `AAAAMMJJHHMMSS` puis une éventuelle variante.
fn lire_compact(reste: &str) -> Option<(Option<NaiveDateTime>, Option<String>)> {
    if reste.len() < 14 || !chiffres(&reste[..14]) {
        return None;
    }
    let n = |d: usize, f: usize| nombre(&reste[d..f]).unwrap_or(0);
    let h = horodatage(n(0, 4), n(4, 6), n(6, 8), n(8, 10), n(10, 12), n(12, 14));
    Some((h, separer_variante(&reste[14..])))
}

/// Lit `AAAAMMJJ_HHMMSS` puis une éventuelle variante.
fn lire_souligne(reste: &str) -> Option<(Option<NaiveDateTime>, Option<String>)> {
    if reste.len() < 15 || !chiffres(&reste[..8]) || &reste[8..9] != "_" || !chiffres(&reste[9..15])
    {
        return None;
    }
    let n = |d: usize, f: usize| nombre(&reste[d..f]).unwrap_or(0);
    let h = horodatage(n(0, 4), n(4, 6), n(6, 8), n(9, 11), n(11, 13), n(13, 15));
    Some((h, separer_variante(&reste[15..])))
}

/// Analyse le nom d'un fichier média.
pub fn analyser(nom_fichier: &str) -> NomAnalyse {
    let base = match nom_fichier.rsplit_once('.') {
        Some((b, _)) if !b.is_empty() => b,
        _ => nom_fichier,
    };
    let identifiant = normaliser(base);
    let normalise = identifiant != base;
    let majuscules = base.to_ascii_uppercase();

    let mut convention = Convention::Autre;
    let mut horo = None;
    let mut variante = None;

    // IMG-AAAAMMJJ-WAnnnn : messagerie, aucun EXIF (C10). Testé en premier,
    // le tiret le distinguant sans ambiguïté des autres motifs en IMG.
    if let Some(reste) = majuscules.strip_prefix("IMG-") {
        if reste.len() >= 8 && chiffres(&reste[..8]) {
            let n = |d: usize, f: usize| nombre(&reste[d..f]).unwrap_or(0);
            if let Some(date) = NaiveDate::from_ymd_opt(n(0, 4) as i32, n(4, 6), n(6, 8)) {
                convention = Convention::Messagerie;
                // Aucune heure dans ce nom : minuit, faute de mieux. La
                // journée est ce qui compte, pas l'heure.
                horo = Some(date.and_time(NaiveTime::MIN));
            }
        }
    }

    if convention == Convention::Autre {
        for prefixe in ["IMG", "VID"] {
            let Some(reste) = majuscules.strip_prefix(prefixe) else {
                continue;
            };
            if let Some((h, v)) = lire_compact(reste) {
                convention = Convention::Telephone;
                horo = h;
                variante = v;
                break;
            }
            if let Some(reste) = reste.strip_prefix('_') {
                if let Some((h, v)) = lire_souligne(reste) {
                    convention = Convention::Repartage;
                    horo = h;
                    variante = v;
                    break;
                }
            }
        }
    }

    // PXL_AAAAMMJJ_HHMMSS, convention Pixel, même statut que Repartage.
    if convention == Convention::Autre {
        if let Some(reste) = majuscules.strip_prefix("PXL_") {
            if let Some((h, v)) = lire_souligne(reste) {
                convention = Convention::Repartage;
                horo = h;
                variante = v;
            }
        }
    }

    if convention == Convention::Autre {
        if let Some(reste) = majuscules.strip_prefix("GOPR") {
            if chiffres(reste) {
                convention = Convention::GoPro;
            }
        }
    }

    // GX010123, GH010123 : GoPro chaptré. Deux lettres, puis le numéro de
    // chapitre et celui de la séquence.
    if convention == Convention::Autre && majuscules.len() > 2 {
        let (tete, reste) = majuscules.split_at(2);
        let mut lettres = tete.bytes();
        if lettres.next() == Some(b'G')
            && lettres.next().is_some_and(|b| b.is_ascii_uppercase())
            && chiffres(reste)
        {
            convention = Convention::GoPro;
        }
    }

    if convention == Convention::Autre {
        if let Some(reste) = majuscules.strip_prefix('P') {
            if chiffres(reste) && reste.len() >= 6 {
                convention = Convention::Compact;
            }
        }
    }

    if convention == Convention::Autre {
        for prefixe in ["DSCN", "DSCF", "_DSC", "DSC_", "DSC", "PICT"] {
            if let Some(reste) = majuscules.strip_prefix(prefixe) {
                if chiffres(reste) {
                    convention = Convention::AppareilPhoto;
                    break;
                }
            }
        }
    }

    NomAnalyse {
        identifiant,
        convention,
        variante,
        horodatage: horo,
        normalise,
    }
}

/// Deux fichiers distincts qui produiraient le même identifiant.
#[derive(Debug, Clone)]
pub struct Collision {
    pub identifiant: String,
    pub fichiers: Vec<PathBuf>,
}

/// Regroupe les fichiers par identifiant et renvoie les groupes de plus d'un.
///
/// Une collision n'est jamais arbitrée automatiquement : elle fait échouer
/// `carnet scan`. Voir SPEC.md, section 5.3.
pub fn detecter_collisions(entrees: &[(String, PathBuf)]) -> Vec<Collision> {
    let mut par_identifiant: BTreeMap<&str, Vec<PathBuf>> = BTreeMap::new();
    for (identifiant, chemin) in entrees {
        par_identifiant
            .entry(identifiant.as_str())
            .or_default()
            .push(chemin.clone());
    }
    par_identifiant
        .into_iter()
        .filter(|(_, fichiers)| fichiers.len() > 1)
        .map(|(identifiant, fichiers)| Collision {
            identifiant: identifiant.to_string(),
            fichiers,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> Option<NaiveDateTime> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()
    }

    #[test]
    fn convention_du_telephone() {
        let a = analyser("IMG20260814151616.jpg");
        assert_eq!(a.convention, Convention::Telephone);
        assert_eq!(a.identifiant, "IMG20260814151616");
        assert_eq!(a.horodatage, date("2026-08-14 15:16:16"));
        assert!(!a.normalise);
        assert!(a.variante.is_none());
    }

    #[test]
    fn convention_video() {
        let a = analyser("VID20260725164207.mp4");
        assert_eq!(a.convention, Convention::Telephone);
        assert_eq!(a.horodatage, date("2026-07-25 16:42:07"));
    }

    /// C3 : le nom porte la date du partage, l'EXIF porte la vraie date.
    /// Le module de nommage ne fait que lire, c'est la datation qui tranche.
    #[test]
    fn fichier_repartage() {
        let a = analyser("IMG_20260730_071148.jpg");
        assert_eq!(a.convention, Convention::Repartage);
        assert_eq!(a.identifiant, "IMG_20260730_071148");
        assert_eq!(a.horodatage, date("2026-07-30 07:11:48"));
        assert!(!a.normalise);
    }

    /// C10 : fichier reçu par messagerie, aucun EXIF, date à la journée.
    #[test]
    fn fichier_de_messagerie() {
        let a = analyser("IMG-20250722-WA0000.jpg");
        assert_eq!(a.convention, Convention::Messagerie);
        assert_eq!(a.horodatage, date("2025-07-22 00:00:00"));
        assert!(a.convention.porte_une_date());
    }

    /// C6 : le tilde est remplacé, la variante est conservée.
    #[test]
    fn nom_au_tilde() {
        let a = analyser("IMG20260808113008~2.jpg");
        assert_eq!(a.identifiant, "IMG20260808113008-2");
        assert_eq!(a.convention, Convention::Telephone);
        assert_eq!(a.variante.as_deref(), Some("2"));
        assert!(a.normalise);
        assert_eq!(a.horodatage, date("2026-08-08 11:30:08"));
    }

    /// C6 : `_01` est déjà valide, l'identifiant ne change pas, et surtout il
    /// n'entre pas en collision avec celui de la photo dont il est la variante.
    #[test]
    fn variante_soulignee_sans_collision() {
        let a = analyser("IMG20260807151640_01.jpg");
        let b = analyser("IMG20260807151640.jpg");
        assert_eq!(a.identifiant, "IMG20260807151640_01");
        assert_eq!(a.variante.as_deref(), Some("01"));
        assert!(!a.normalise);
        assert_ne!(a.identifiant, b.identifiant);
    }

    /// C4 : les GoPro ne portent aucune date dans leur nom.
    #[test]
    fn fichiers_gopro() {
        let a = analyser("GOPR2699.JPG");
        assert_eq!(a.convention, Convention::GoPro);
        assert!(a.horodatage.is_none());
        assert!(!a.convention.porte_une_date());
        assert_eq!(analyser("GX010123.MP4").convention, Convention::GoPro);
    }

    #[test]
    fn appareils_de_la_polynesie() {
        assert_eq!(analyser("P1000123.JPG").convention, Convention::Compact);
        assert_eq!(analyser("DSCN0001.JPG").convention, Convention::AppareilPhoto);
    }

    #[test]
    fn nom_sans_convention() {
        let a = analyser("photo de vacances (1).jpg");
        assert_eq!(a.convention, Convention::Autre);
        assert_eq!(a.identifiant, "photo-de-vacances--1-");
        assert!(a.normalise);
        assert!(a.horodatage.is_none());
    }

    #[test]
    fn date_impossible_dans_le_nom() {
        let a = analyser("IMG20261345999999.jpg");
        assert!(a.horodatage.is_none());
    }

    /// C8 : deux fichiers homonymes dans deux dossiers différents.
    #[test]
    fn collisions_detectees() {
        let entrees = vec![
            (
                "IMG20260731092009".to_string(),
                PathBuf::from("IMG20260731092009.jpg"),
            ),
            (
                "IMG20260731092009".to_string(),
                PathBuf::from("[Originals]/IMG20260731092009.jpg"),
            ),
            (
                "IMG20260814151616".to_string(),
                PathBuf::from("IMG20260814151616.jpg"),
            ),
        ];
        let collisions = detecter_collisions(&entrees);
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].identifiant, "IMG20260731092009");
        assert_eq!(collisions[0].fichiers.len(), 2);
    }

    #[test]
    fn extensions_reconnues() {
        assert!(est_media(Path::new("a.JPG")));
        assert!(est_media(Path::new("a.mp4")));
        assert!(!est_media(Path::new("a.txt")));
        assert!(est_video(Path::new("a.MP4")));
        assert!(!est_video(Path::new("a.jpg")));
    }
}
