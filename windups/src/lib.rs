//! Shared library for all windups. One module per concern.
pub mod base;
pub mod conflicts;
pub mod db;
pub mod decisions;
pub mod handoff;
pub mod lifecycle;
pub mod names;
pub mod paths;
pub mod proposals;
pub mod prune;
pub mod registry;
pub mod returns;
pub mod settings;
pub mod unresolved;
pub mod vectors;
pub mod works;

pub use settings::Settings;
