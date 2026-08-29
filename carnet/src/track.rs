//! Vocabulaire des traces : modes de déplacement, couleurs, inférence.
//!
//! Voir SPEC.md, sections 5.6 et D6. La construction des `LineString` arrive
//! plus loin dans le lot 2 ; ce module commence par le vocabulaire, dont les
//! surcharges ont besoin.

use serde::{Deserialize, Serialize};

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
    /// Palette de la section 9.5 : neutres chauds, accent terre cuite. Le
    /// terre cuite est réservé à la marche, qui est le mode le plus raconté.
    pub fn couleur(self) -> &'static str {
        match self {
            Mode::Marche => "#c0562a",
            Mode::Route => "#8a6a4f",
            Mode::Velo => "#4f7d5e",
            Mode::Bateau => "#3f6d8c",
            Mode::Train => "#7a5c8a",
            Mode::Telepherique => "#a8873f",
        }
    }

    /// Seuls les tronçons routiers partent au moteur d'itinéraire.
    ///
    /// C'est la garde de D6 : map-matcher une randonnée la ferait suivre les
    /// départementales. Un test du lot 2 vérifie qu'aucun autre mode ne passe.
    pub fn calculable(self) -> bool {
        matches!(self, Mode::Route)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seule_la_route_part_au_calcul() {
        assert!(Mode::Route.calculable());
        for mode in [
            Mode::Marche,
            Mode::Velo,
            Mode::Bateau,
            Mode::Train,
            Mode::Telepherique,
        ] {
            assert!(!mode.calculable(), "{} ne doit jamais être calculé", mode.nom());
        }
    }

    #[test]
    fn couleur_de_la_marche_conforme_a_la_spec() {
        assert_eq!(Mode::Marche.couleur(), "#c0562a");
    }

    #[test]
    fn inference_par_la_vitesse() {
        assert_eq!(Mode::depuis_vitesse(3.5), Mode::Marche);
        assert_eq!(Mode::depuis_vitesse(15.0), Mode::Velo);
        assert_eq!(Mode::depuis_vitesse(70.0), Mode::Route);
    }

    #[test]
    fn lecture_depuis_le_yaml() {
        let m: Mode = serde_norway::from_str("bateau").expect("mode lisible");
        assert_eq!(m, Mode::Bateau);
    }
}
