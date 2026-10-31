//! Agent du poste élève (IMPL.md, « Architecture »).
//!
//! - [`setup`] : rattachement par le paquet, identité du poste,
//!   installation dans le compte de l'utilisateur (SPEC.md, chap. 7) ;
//! - [`core`] : connexion au relais, séance, capture de l'écran
//!   (SPEC.md, chap. 1) ;
//! - [`restrict`] : restrictions de la séance (SPEC.md, chap. 2), dont le
//!   filtrage web par le proxy local [`webfilter`].
//!
//! L'affichage (indicateur de séance, identification) est fourni par
//! l'application, derrière le trait [`core::Ui`].

pub mod core;
pub mod restrict;
pub mod setup;
pub mod webfilter;
