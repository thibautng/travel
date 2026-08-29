//! Itinéraires calculés, et leur cache versionné. Voir D6.
//!
//! Il n'existe aucun enregistrement GPS continu du voyage : relier deux
//! positions de photos par une droite ferait traverser les massifs. La route
//! et le vélo partent donc au moteur, chacun sur son réseau, et le
//! résultat est figé dans `data/<voyage>/itineraires.json`.
//!
//! Le cache est consulté avant tout appel réseau. Une fois peuplé, il rend le
//! build reproductible hors ligne et sans dépendance à un service tiers.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::scan::Position;
use crate::track::Mode;

/// Variable d'environnement portant la clé OpenRouteService.
pub const VARIABLE_CLE: &str = "CARNET_ORS_CLE";

fn url_ors(profil: &str) -> String {
    format!("https://api.openrouteservice.org/v2/directions/{profil}/geojson")
}

/// Pause entre deux appels réseau. Le palier gratuit d'OpenRouteService
/// plafonne à 40 requêtes par minute ; une seconde et demie laisse de la marge.
const PAUSE_ENTRE_APPELS: std::time::Duration = std::time::Duration::from_millis(1500);

#[derive(Debug, thiserror::Error)]
pub enum ErreurItineraire {
    #[error("lecture de {chemin} impossible")]
    Lecture {
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

    #[error("{chemin} est illisible : le cache d'itinéraires est corrompu")]
    CacheCorrompu {
        chemin: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("le mode « {0} » ne doit jamais partir au moteur d'itinéraire (D6)")]
    ModeNonCalculable(&'static str),

    #[error("le moteur d'itinéraire a répondu {code}")]
    Refus { code: u16 },

    #[error("réponse du moteur d'itinéraire inexploitable")]
    ReponseInvalide,

    #[error("appel au moteur d'itinéraire impossible")]
    Reseau {
        #[source]
        source: Box<ureq::Error>,
    },
}

/// Un tronçon calculé, tel qu'il est mis en cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajet {
    /// Points en ordre GeoJSON, `[longitude, latitude]`.
    pub points: Vec<[f64; 2]>,
    pub distance_m: f64,
    pub duree_s: f64,
    /// Moteur qui a produit ce trajet, pour savoir quoi réinterroger le jour
    /// où l'on en change.
    pub moteur: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Contenu {
    version: u32,
    #[serde(default)]
    entrees: BTreeMap<String, Trajet>,
}

/// Ce qu'a donné une demande d'itinéraire.
#[derive(Debug)]
pub enum Resolution {
    /// Trouvé dans le cache, aucun appel réseau.
    Cache(Trajet),
    /// Calculé à l'instant, et ajouté au cache.
    Calcule(Trajet),
    /// Pas de cache et pas de clé : le tronçon restera une droite.
    Indisponible,
}

pub struct Itineraires {
    chemin: PathBuf,
    contenu: Contenu,
    cle: Option<String>,
    modifie: bool,
    pub appels: usize,
    pub manques: usize,
}

/// Clé de cache : mode, extrémités et points de passage, arrondis à la
/// cinquième décimale, soit environ un mètre. Deux photos prises au même
/// endroit ne déclenchent pas deux appels.
fn cle_cache(mode: Mode, depart: &Position, arrivee: &Position, passages: &[[f64; 2]]) -> String {
    let mut cle = format!(
        "{}|{:.5},{:.5}|{:.5},{:.5}",
        mode.nom(),
        depart.lon,
        depart.lat,
        arrivee.lon,
        arrivee.lat
    );
    for point in passages {
        cle.push_str(&format!("|{:.5},{:.5}", point[0], point[1]));
    }
    cle
}

impl Itineraires {
    /// Charge le cache. Son absence est normale au premier build.
    pub fn charger(depot: &Path, voyage_id: &str) -> Result<Self, ErreurItineraire> {
        let chemin = depot
            .join("data")
            .join(voyage_id)
            .join("itineraires.json");
        let contenu = if chemin.is_file() {
            let texte =
                std::fs::read_to_string(&chemin).map_err(|source| ErreurItineraire::Lecture {
                    chemin: chemin.clone(),
                    source,
                })?;
            serde_json::from_str(&texte).map_err(|source| ErreurItineraire::CacheCorrompu {
                chemin: chemin.clone(),
                source,
            })?
        } else {
            Contenu {
                version: 1,
                entrees: BTreeMap::new(),
            }
        };
        Ok(Self {
            chemin,
            contenu,
            cle: std::env::var(VARIABLE_CLE).ok().filter(|c| !c.trim().is_empty()),
            modifie: false,
            appels: 0,
            manques: 0,
        })
    }

    pub fn cle_presente(&self) -> bool {
        self.cle.is_some()
    }

    pub fn taille_cache(&self) -> usize {
        self.contenu.entrees.len()
    }

    /// Résout un tronçon routier, cache d'abord.
    pub fn resoudre(
        &mut self,
        mode: Mode,
        depart: &Position,
        arrivee: &Position,
        passages: &[[f64; 2]],
    ) -> Result<Resolution, ErreurItineraire> {
        // Garde de D6. Une randonnée map-matchée suivrait les départementales.
        if !mode.calculable() {
            return Err(ErreurItineraire::ModeNonCalculable(mode.nom()));
        }

        let cle = cle_cache(mode, depart, arrivee, passages);
        if let Some(trajet) = self.contenu.entrees.get(&cle) {
            return Ok(Resolution::Cache(trajet.clone()));
        }

        let Some(cle_api) = self.cle.clone() else {
            self.manques += 1;
            return Ok(Resolution::Indisponible);
        };

        if self.appels > 0 {
            std::thread::sleep(PAUSE_ENTRE_APPELS);
        }
        let profil = mode.profil().ok_or(ErreurItineraire::ModeNonCalculable(mode.nom()))?;
        let trajet = self.appeler(profil, &cle_api, depart, arrivee, passages)?;
        self.appels += 1;
        self.contenu.entrees.insert(cle, trajet.clone());
        self.modifie = true;
        Ok(Resolution::Calcule(trajet))
    }

    fn appeler(
        &self,
        profil: &str,
        cle_api: &str,
        depart: &Position,
        arrivee: &Position,
        passages: &[[f64; 2]],
    ) -> Result<Trajet, ErreurItineraire> {
        let mut coordonnees: Vec<[f64; 2]> = vec![[depart.lon, depart.lat]];
        coordonnees.extend_from_slice(passages);
        coordonnees.push([arrivee.lon, arrivee.lat]);

        let corps = serde_json::json!({ "coordinates": coordonnees });
        let reponse = ureq::post(url_ors(profil))
            .header("Authorization", cle_api)
            .header("Content-Type", "application/json")
            .send_json(&corps);

        let mut reponse = match reponse {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(code)) => {
                return Err(ErreurItineraire::Refus { code })
            }
            Err(source) => {
                return Err(ErreurItineraire::Reseau {
                    source: Box::new(source),
                })
            }
        };

        let json: serde_json::Value = reponse
            .body_mut()
            .read_json()
            .map_err(|_| ErreurItineraire::ReponseInvalide)?;
        lire_reponse(&json, profil).ok_or(ErreurItineraire::ReponseInvalide)
    }

    /// Écrit le cache, s'il a changé.
    pub fn enregistrer(&self) -> Result<Option<&Path>, ErreurItineraire> {
        if !self.modifie {
            return Ok(None);
        }
        if let Some(parent) = self.chemin.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ErreurItineraire::Ecriture {
                chemin: parent.to_path_buf(),
                source,
            })?;
        }
        let texte = serde_json::to_string_pretty(&self.contenu)
            .map_err(|_| ErreurItineraire::ReponseInvalide)?;
        std::fs::write(&self.chemin, texte).map_err(|source| ErreurItineraire::Ecriture {
            chemin: self.chemin.clone(),
            source,
        })?;
        Ok(Some(&self.chemin))
    }
}

/// Extrait la géométrie de la réponse GeoJSON d'OpenRouteService.
fn lire_reponse(json: &serde_json::Value, profil: &str) -> Option<Trajet> {
    let feature = json.get("features")?.as_array()?.first()?;
    let coordonnees = feature.get("geometry")?.get("coordinates")?.as_array()?;
    let mut points = Vec::with_capacity(coordonnees.len());
    for point in coordonnees {
        let paire = point.as_array()?;
        points.push([paire.first()?.as_f64()?, paire.get(1)?.as_f64()?]);
    }
    if points.len() < 2 {
        return None;
    }
    let resume = feature
        .get("properties")
        .and_then(|p| p.get("summary"));
    Some(Trajet {
        points,
        distance_m: resume
            .and_then(|r| r.get("distance"))
            .and_then(|d| d.as_f64())
            .unwrap_or(0.0),
        duree_s: resume
            .and_then(|r| r.get("duration"))
            .and_then(|d| d.as_f64())
            .unwrap_or(0.0),
        moteur: format!("openrouteservice/{profil}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(lat: f64, lon: f64) -> Position {
        Position { lat, lon, alt: None }
    }

    /// La garde de D6 : aucun mode autre que la route ne part au calcul.
    #[test]
    fn refuse_tout_mode_non_routier() {
        let mut itineraires = Itineraires {
            chemin: PathBuf::from("inexistant.json"),
            contenu: Contenu::default(),
            cle: Some("fausse".to_string()),
            modifie: false,
            appels: 0,
            manques: 0,
        };
        for mode in [Mode::Marche, Mode::Bateau, Mode::Train, Mode::Telepherique] {
            let erreur = itineraires
                .resoudre(mode, &position(45.0, 7.0), &position(45.1, 7.1), &[])
                .expect_err("le mode doit être refusé");
            assert!(matches!(erreur, ErreurItineraire::ModeNonCalculable(_)));
        }
    }

    #[test]
    fn le_cache_evite_l_appel_reseau() {
        let mut contenu = Contenu {
            version: 1,
            entrees: BTreeMap::new(),
        };
        let cle = cle_cache(Mode::Route, &position(45.0, 7.0), &position(45.1, 7.1), &[]);
        contenu.entrees.insert(
            cle,
            Trajet {
                points: vec![[7.0, 45.0], [7.1, 45.1]],
                distance_m: 100.0,
                duree_s: 10.0,
                moteur: "test".to_string(),
            },
        );
        let mut itineraires = Itineraires {
            chemin: PathBuf::from("inexistant.json"),
            contenu,
            // Aucune clé : si le cache ne répondait pas, ce serait Indisponible.
            cle: None,
            modifie: false,
            appels: 0,
            manques: 0,
        };
        let resolution = itineraires
            .resoudre(Mode::Route, &position(45.0, 7.0), &position(45.1, 7.1), &[])
            .expect("résolution");
        assert!(matches!(resolution, Resolution::Cache(_)));
        assert_eq!(itineraires.appels, 0);
    }

    #[test]
    fn sans_cle_ni_cache_le_troncon_reste_indisponible() {
        let mut itineraires = Itineraires {
            chemin: PathBuf::from("inexistant.json"),
            contenu: Contenu::default(),
            cle: None,
            modifie: false,
            appels: 0,
            manques: 0,
        };
        let resolution = itineraires
            .resoudre(Mode::Route, &position(45.0, 7.0), &position(45.1, 7.1), &[])
            .expect("résolution");
        assert!(matches!(resolution, Resolution::Indisponible));
        assert_eq!(itineraires.manques, 1);
    }

    /// Les points de passage font partie de la clé : forcer un col plutôt
    /// qu'un tunnel doit produire une entrée distincte.
    #[test]
    fn les_points_de_passage_changent_la_cle() {
        let sans = cle_cache(Mode::Route, &position(45.0, 7.0), &position(45.1, 7.1), &[]);
        let avec = cle_cache(
            Mode::Route,
            &position(45.0, 7.0),
            &position(45.1, 7.1),
            &[[7.05, 45.05]],
        );
        assert_ne!(sans, avec);
    }

    #[test]
    fn lecture_d_une_reponse_geojson() {
        let json = serde_json::json!({
            "features": [{
                "geometry": { "coordinates": [[7.0, 45.0], [7.05, 45.05], [7.1, 45.1]] },
                "properties": { "summary": { "distance": 12345.6, "duration": 987.0 } }
            }]
        });
        let trajet = lire_reponse(&json, "driving-car").expect("réponse lisible");
        assert_eq!(trajet.points.len(), 3);
        assert!((trajet.distance_m - 12345.6).abs() < 1e-6);
    }

    #[test]
    fn reponse_sans_geometrie_refusee() {
        let json = serde_json::json!({ "features": [] });
        assert!(lire_reponse(&json, "driving-car").is_none());
    }
}
