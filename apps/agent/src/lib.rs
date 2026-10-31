//! Agent du poste élève (IMPL.md, « Architecture »).
//!
//! - [`setup`] : rattachement par le paquet, identité du poste,
//!   installation dans le compte de l'utilisateur (SPEC.md, chap. 7) ;
//! - [`core`] : connexion au relais, séance, capture de l'écran
//!   (SPEC.md, chap. 1).
//!
//! L'affichage (indicateur de séance, identification) est fourni par
//! l'application, derrière le trait [`core::Ui`].

pub mod core;
pub mod setup;
