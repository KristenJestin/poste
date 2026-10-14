//! Bibliothèque commune de `poste` : chemins XDG, configuration,
//! découverte des modules, notifications et petit utilitaire de temps.

pub mod config;
pub mod env;
pub mod modules;
pub mod notify;
pub mod time;

pub use config::{Config, Source};
pub use env::Env;
pub use modules::{Card, Module, Origin};
