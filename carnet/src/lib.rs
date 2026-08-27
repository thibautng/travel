//! Pipeline médias du site des voyages. Voir SPEC.md, section 6.
//!
//! Les modules sont exposés en bibliothèque pour que les tests d'intégration
//! de `tests/contraintes.rs` puissent les exercer avec les fixtures. Le
//! binaire `carnet` n'est qu'une fine couche de ligne de commande par-dessus.

pub mod emit;
pub mod noms;
pub mod quality;
pub mod scan;
pub mod voyage;
