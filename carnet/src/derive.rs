//! Génération des dérivés d'images. Voir SPEC.md, section 6.2, étapes 9 et 10.
//!
//! Trois largeurs, un repli, un aperçu. Le format est un paramètre du
//! pipeline et non une constante : l'AVIF est la cible, mais son encodeur
//! s'appuie lourdement sur AVX2, absent des machines d'avant 2013.
//!
//! Ce module ne lit jamais le dossier source en écriture. Il n'écrit que
//! dans `media/<voyage>/`.

use fast_image_resize::images::Image;
use fast_image_resize::{IntoImageView, ResizeOptions, Resizer};
use image::{DynamicImage, ImageEncoder, ImageReader};
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::scan::{Media, TypeMedia};

/// Largeurs produites, de la vignette au grand format.
pub const LARGEURS: [u32; 3] = [320, 1024, 2048];

/// Largeur du repli, le seul format de secours produit.
pub const LARGEUR_REPLI: u32 = 1024;

/// Largeur de l'aperçu inline. Seize pixels suffisent à donner la couleur
/// et la composition, pour quelques centaines d'octets.
pub const LARGEUR_LQIP: u32 = 16;

#[derive(Debug, thiserror::Error)]
pub enum ErreurDerive {
    #[error("lecture de {chemin} impossible")]
    Lecture {
        chemin: PathBuf,
        #[source]
        source: image::ImageError,
    },

    #[error("écriture de {chemin} impossible")]
    Ecriture {
        chemin: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("redimensionnement de {chemin} impossible")]
    Redimension { chemin: PathBuf },

    #[error("encodage de {chemin} impossible")]
    Encodage { chemin: PathBuf },
}

/// Format des dérivés.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Cible du projet. Encodeur `ravif`, lent sans AVX2.
    Avif,
    /// Valeur de repli sur une machine sans AVX2. Environ 25 % plus lourd.
    Jpeg,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Avif => "avif",
            Format::Jpeg => "jpg",
        }
    }

    pub fn depuis_nom(nom: &str) -> Option<Self> {
        match nom.to_ascii_lowercase().as_str() {
            "avif" => Some(Format::Avif),
            "jpeg" | "jpg" => Some(Format::Jpeg),
            _ => None,
        }
    }
}

/// Paramètres d'encodage. Leur empreinte entre dans le cache de build : un
/// changement de qualité invalide les dérivés existants.
#[derive(Debug, Clone, Copy)]
pub struct Reglages {
    pub format: Format,
    /// Qualité, de 1 à 100.
    pub qualite: u8,
    /// Vitesse d'encodage AVIF, de 1 (lent et dense) à 10 (rapide).
    pub vitesse_avif: u8,
}

impl Default for Reglages {
    fn default() -> Self {
        Self {
            format: Format::Avif,
            qualite: 72,
            vitesse_avif: 6,
        }
    }
}

impl Reglages {
    /// Empreinte des paramètres, pour le cache de build (étape 14).
    pub fn empreinte(&self) -> String {
        format!(
            "v2|{}|q{}|s{}|{}x{}x{}|repli{}",
            self.format.extension(),
            self.qualite,
            self.vitesse_avif,
            LARGEURS[0],
            LARGEURS[1],
            LARGEURS[2],
            LARGEUR_REPLI
        )
    }
}

/// Ce qu'une photo a produit.
///
/// Les dimensions sont relevées ici et non au scan : le téléphone ne les
/// écrit pas dans l'EXIF, seules 16 photos sur 705 en portaient. Or ce module
/// décode déjà chaque image pour la redimensionner, et il les connaît donc
/// après redressement, ce qui est la valeur utile au site.
#[derive(Debug, Clone)]
pub struct Production {
    pub derives: Derives,
    pub lqip: String,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Chemins des dérivés, relatifs à `media/<voyage>/`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Derives {
    pub vignette: String,
    pub moyen: String,
    pub grand: String,
    /// Filet de sécurité pour les appareils anciens, en JPEG.
    pub repli: String,
}

/// Applique l'orientation EXIF, puis oublie qu'elle a existé.
///
/// Les dérivés sont écrits droits et sans métadonnées : un navigateur qui
/// ignorerait l'orientation afficherait sinon des photos couchées.
fn redresser(image: DynamicImage, orientation: Option<u16>) -> DynamicImage {
    match orientation.unwrap_or(1) {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate270().fliph(),
        8 => image.rotate270(),
        _ => image,
    }
}

/// Redimensionne à la largeur voulue, en conservant les proportions.
///
/// Une image plus petite que la cible n'est jamais agrandie : mieux vaut un
/// dérivé plus petit que du flou.
fn redimensionner(
    source: &DynamicImage,
    largeur: u32,
    chemin: &Path,
) -> Result<DynamicImage, ErreurDerive> {
    if source.width() <= largeur {
        return Ok(source.clone());
    }
    let hauteur = ((source.height() as f64) * (largeur as f64) / (source.width() as f64))
        .round()
        .max(1.0) as u32;

    let type_pixel = source
        .pixel_type()
        .ok_or_else(|| ErreurDerive::Redimension {
            chemin: chemin.to_path_buf(),
        })?;
    let mut cible = Image::new(largeur, hauteur, type_pixel);
    Resizer::new()
        .resize(source, &mut cible, &ResizeOptions::new())
        .map_err(|_| ErreurDerive::Redimension {
            chemin: chemin.to_path_buf(),
        })?;

    let brut = cible.into_vec();
    let image = match type_pixel {
        fast_image_resize::PixelType::U8x3 => image::RgbImage::from_raw(largeur, hauteur, brut)
            .map(DynamicImage::ImageRgb8),
        fast_image_resize::PixelType::U8x4 => image::RgbaImage::from_raw(largeur, hauteur, brut)
            .map(DynamicImage::ImageRgba8),
        _ => None,
    };
    image.ok_or_else(|| ErreurDerive::Redimension {
        chemin: chemin.to_path_buf(),
    })
}

fn encoder_jpeg(image: &DynamicImage, qualite: u8, chemin: &Path) -> Result<(), ErreurDerive> {
    let rgb = image.to_rgb8();
    let mut octets: Vec<u8> = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut octets, qualite)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|_| ErreurDerive::Encodage {
            chemin: chemin.to_path_buf(),
        })?;
    ecrire(chemin, &octets)
}

fn encoder_avif(
    image: &DynamicImage,
    reglages: &Reglages,
    chemin: &Path,
) -> Result<(), ErreurDerive> {
    let rgb = image.to_rgb8();
    let pixels: Vec<rgb::RGB8> = rgb
        .pixels()
        .map(|p| rgb::RGB8::new(p[0], p[1], p[2]))
        .collect();
    let tampon = imgref::Img::new(pixels, rgb.width() as usize, rgb.height() as usize);
    let resultat = ravif::Encoder::new()
        .with_quality(reglages.qualite as f32)
        .with_speed(reglages.vitesse_avif)
        .encode_rgb(tampon.as_ref())
        .map_err(|_| ErreurDerive::Encodage {
            chemin: chemin.to_path_buf(),
        })?;
    ecrire(chemin, &resultat.avif_file)
}

fn ecrire(chemin: &Path, octets: &[u8]) -> Result<(), ErreurDerive> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ErreurDerive::Ecriture {
            chemin: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(chemin, octets).map_err(|source| ErreurDerive::Ecriture {
        chemin: chemin.to_path_buf(),
        source,
    })
}

/// Encodage base64 standard, écrit à la main.
///
/// Trente lignes contre une dépendance de plus, pour un usage unique : la
/// chaîne d'aperçu inline du JSON (SPEC.md, section 12).
fn base64(octets: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut sortie = String::with_capacity(octets.len().div_ceil(3) * 4);
    for morceau in octets.chunks(3) {
        let a = morceau[0] as u32;
        let b = morceau.get(1).copied().unwrap_or(0) as u32;
        let c = morceau.get(2).copied().unwrap_or(0) as u32;
        let bloc = (a << 16) | (b << 8) | c;
        sortie.push(TABLE[(bloc >> 18) as usize & 63] as char);
        sortie.push(TABLE[(bloc >> 12) as usize & 63] as char);
        sortie.push(if morceau.len() > 1 {
            TABLE[(bloc >> 6) as usize & 63] as char
        } else {
            '='
        });
        sortie.push(if morceau.len() > 2 {
            TABLE[bloc as usize & 63] as char
        } else {
            '='
        });
    }
    sortie
}

/// Aperçu inline, en JPEG, encodé en base64.
fn lqip(image: &DynamicImage, chemin: &Path) -> Result<String, ErreurDerive> {
    let petit = redimensionner(image, LARGEUR_LQIP, chemin)?;
    let rgb = petit.to_rgb8();
    let mut octets: Vec<u8> = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut octets, 40)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|_| ErreurDerive::Encodage {
            chemin: chemin.to_path_buf(),
        })?;
    Ok(format!("data:image/jpeg;base64,{}", base64(&octets)))
}

/// Produit les dérivés d'une photo et son aperçu.
///
/// `dossier` est `media/<voyage>/`. Les chemins renvoyés lui sont relatifs.
pub fn produire(
    media: &Media,
    source: &Path,
    dossier: &Path,
    reglages: &Reglages,
) -> Result<Production, ErreurDerive> {
    debug_assert_eq!(media.type_media, TypeMedia::Photo);

    let originale = ImageReader::open(source)
        .map_err(|source_erreur| ErreurDerive::Ecriture {
            chemin: source.to_path_buf(),
            source: source_erreur,
        })?
        .with_guessed_format()
        .map_err(|source_erreur| ErreurDerive::Ecriture {
            chemin: source.to_path_buf(),
            source: source_erreur,
        })?
        .decode()
        .map_err(|source_erreur| ErreurDerive::Lecture {
            chemin: source.to_path_buf(),
            source: source_erreur,
        })?;
    let originale = redresser(originale, media.orientation);

    let ext = reglages.format.extension();
    let mut chemins = Vec::with_capacity(3);
    for largeur in LARGEURS {
        let relatif = format!("photos/{}-{}.{}", media.id, largeur, ext);
        let chemin = dossier.join(&relatif);
        let reduite = redimensionner(&originale, largeur, &chemin)?;
        match reglages.format {
            Format::Avif => encoder_avif(&reduite, reglages, &chemin)?,
            Format::Jpeg => encoder_jpeg(&reduite, reglages.qualite, &chemin)?,
        }
        chemins.push(relatif);
    }

    // Repli JPEG, en une seule taille : un filet de sécurité, pas un jeu complet.
    let relatif_repli = format!("photos/{}-{}.jpg", media.id, LARGEUR_REPLI);
    if reglages.format != Format::Jpeg {
        let chemin = dossier.join(&relatif_repli);
        let reduite = redimensionner(&originale, LARGEUR_REPLI, &chemin)?;
        encoder_jpeg(&reduite, reglages.qualite, &chemin)?;
    }

    let apercu = lqip(&originale, &dossier.join("lqip"))?;

    Ok(Production {
        derives: Derives {
            vignette: chemins[0].clone(),
            moyen: chemins[1].clone(),
            grand: chemins[2].clone(),
            repli: relatif_repli,
        },
        lqip: apercu,
        largeur: originale.width(),
        hauteur: originale.height(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_test(largeur: u32, hauteur: u32) -> DynamicImage {
        let mut brut = image::RgbImage::new(largeur, hauteur);
        for (x, y, pixel) in brut.enumerate_pixels_mut() {
            *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        DynamicImage::ImageRgb8(brut)
    }

    #[test]
    fn base64_connu() {
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Mane"), "TWFuZQ==");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn redimension_conserve_les_proportions() {
        let source = image_test(4000, 3000);
        let reduite = redimensionner(&source, 1024, Path::new("test")).expect("redimension");
        assert_eq!(reduite.width(), 1024);
        assert_eq!(reduite.height(), 768);
    }

    /// Une image plus petite que la cible ne doit pas être agrandie : un
    /// dérivé plus petit vaut mieux qu'un dérivé flou.
    #[test]
    fn pas_d_agrandissement() {
        let source = image_test(800, 600);
        let reduite = redimensionner(&source, 2048, Path::new("test")).expect("redimension");
        assert_eq!(reduite.width(), 800);
    }

    #[test]
    fn orientation_appliquee() {
        let source = image_test(100, 50);
        // 6 : rotation de 90 degrés, les dimensions s'échangent.
        let droite = redresser(source.clone(), Some(6));
        assert_eq!((droite.width(), droite.height()), (50, 100));
        // 1 : rien à faire.
        let inchangee = redresser(source, Some(1));
        assert_eq!((inchangee.width(), inchangee.height()), (100, 50));
    }

    #[test]
    fn apercu_inline_et_leger() {
        let source = image_test(4000, 3000);
        let apercu = lqip(&source, Path::new("test")).expect("aperçu");
        assert!(apercu.starts_with("data:image/jpeg;base64,"));
        assert!(
            apercu.len() < 2000,
            "un aperçu de 16 pixels ne doit pas peser {} octets",
            apercu.len()
        );
    }

    /// L'empreinte entre dans le cache de build : deux réglages différents ne
    /// doivent jamais produire la même.
    #[test]
    fn empreinte_distingue_les_reglages() {
        let a = Reglages::default();
        let b = Reglages {
            qualite: 90,
            ..Reglages::default()
        };
        let c = Reglages {
            format: Format::Jpeg,
            ..Reglages::default()
        };
        assert_ne!(a.empreinte(), b.empreinte());
        assert_ne!(a.empreinte(), c.empreinte());
    }

    #[test]
    fn format_depuis_le_nom() {
        assert_eq!(Format::depuis_nom("AVIF"), Some(Format::Avif));
        assert_eq!(Format::depuis_nom("jpg"), Some(Format::Jpeg));
        assert_eq!(Format::depuis_nom("png"), None);
    }
}

// ---------------------------------------------------------------------------
// Cache de build
// ---------------------------------------------------------------------------

use serde::Deserialize;
use std::collections::BTreeMap;

/// Ce qui identifie un fichier source sans le relire : taille et date de
/// modification. Lire les 8,6 Go pour en calculer une empreinte couterait une
/// lecture complete du disque a chaque build, pour un dossier declare en
/// lecture seule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntreeCache {
    pub octets: u64,
    pub mtime: i64,
    pub derives: Derives,
    pub lqip: String,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Cache de build, ecrit dans `data/<voyage>/.build-cache.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheBuild {
    /// Empreinte des reglages d'encodage. Un changement invalide tout.
    #[serde(default)]
    pub empreinte: String,
    #[serde(default)]
    pub entrees: BTreeMap<String, EntreeCache>,
    #[serde(skip)]
    chemin: PathBuf,
}

/// Taille et date de modification d'un fichier source.
pub fn signature(chemin: &Path) -> Option<(u64, i64)> {
    let meta = std::fs::metadata(chemin).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some((meta.len(), mtime))
}

impl CacheBuild {
    pub fn charger(depot: &Path, voyage_id: &str, reglages: &Reglages) -> Self {
        let chemin = depot
            .join("data")
            .join(voyage_id)
            .join(".build-cache.json");
        let mut cache: CacheBuild = std::fs::read_to_string(&chemin)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();

        // Un changement de reglage invalide les derives existants : ils ont
        // ete produits avec d'autres parametres.
        if cache.empreinte != reglages.empreinte() {
            cache.entrees.clear();
            cache.empreinte = reglages.empreinte();
        }
        cache.chemin = chemin;
        cache
    }

    /// Entree valide pour ce media, si sa source n'a pas bouge et si ses
    /// derives sont toujours sur le disque.
    pub fn valide(&self, id: &str, signature: (u64, i64), dossier: &Path) -> Option<&EntreeCache> {
        let entree = self.entrees.get(id)?;
        if entree.octets != signature.0 || entree.mtime != signature.1 {
            return None;
        }
        let presents = [
            &entree.derives.vignette,
            &entree.derives.moyen,
            &entree.derives.grand,
            &entree.derives.repli,
        ]
        .iter()
        .all(|relatif| dossier.join(relatif).is_file());
        if presents {
            Some(entree)
        } else {
            None
        }
    }

    pub fn inserer(&mut self, id: &str, entree: EntreeCache) {
        self.entrees.insert(id.to_string(), entree);
    }

    pub fn enregistrer(&self) -> Result<(), ErreurDerive> {
        if let Some(parent) = self.chemin.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ErreurDerive::Ecriture {
                chemin: parent.to_path_buf(),
                source,
            })?;
        }
        let texte = serde_json::to_string_pretty(self).map_err(|_| ErreurDerive::Encodage {
            chemin: self.chemin.clone(),
        })?;
        std::fs::write(&self.chemin, texte).map_err(|source| ErreurDerive::Ecriture {
            chemin: self.chemin.clone(),
            source,
        })
    }
}

#[cfg(test)]
mod calibrage {
    use super::*;

    /// Mesure le cout d'encodage d'une vraie photo, dans les deux formats.
    ///
    /// Ignore par defaut : il depend d'un fichier hors du depot. A lancer
    /// pour calibrer une machine, avant de promettre un temps de build.
    ///
    /// ```text
    /// CARNET_PHOTO_TEST=".../IMG20260814151616.jpg" \
    ///   cargo test --release calibrage -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn cout_d_encodage_par_photo() {
        let Ok(chemin) = std::env::var("CARNET_PHOTO_TEST") else {
            eprintln!("CARNET_PHOTO_TEST non definie, mesure sautee");
            return;
        };
        let originale = ImageReader::open(&chemin)
            .expect("photo lisible")
            .decode()
            .expect("photo decodable");
        eprintln!(
            "photo {} x {} pixels",
            originale.width(),
            originale.height()
        );

        let dossier = std::env::temp_dir().join("carnet-calibrage");
        for format in [Format::Jpeg, Format::Avif] {
            let reglages = Reglages {
                format,
                ..Reglages::default()
            };
            let debut = std::time::Instant::now();
            let mut octets = 0u64;
            for largeur in LARGEURS {
                let cible = dossier.join(format!("essai-{largeur}.{}", format.extension()));
                let reduite = redimensionner(&originale, largeur, &cible).expect("redimension");
                match format {
                    Format::Avif => encoder_avif(&reduite, &reglages, &cible).expect("avif"),
                    Format::Jpeg => {
                        encoder_jpeg(&reduite, reglages.qualite, &cible).expect("jpeg")
                    }
                }
                octets += std::fs::metadata(&cible).map(|m| m.len()).unwrap_or(0);
            }
            eprintln!(
                "  {:<5} {:>7.2} s pour les trois tailles, {:>5} Ko",
                format.extension(),
                debut.elapsed().as_secs_f64(),
                octets / 1024
            );
        }
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
