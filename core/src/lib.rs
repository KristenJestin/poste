//! Common library of `poste`: XDG paths, configuration,
//! module discovery, notifications and a small time utility.

pub mod config;
pub mod env;
pub mod modules;
pub mod notify;
pub mod shell;
pub mod time;

pub use config::{Config, Source};
pub use env::Env;
pub use modules::{Card, Module, Origin};
