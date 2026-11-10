//! The shared windup core: what every windup has, so new windups build on it and one client can
//! talk to any of them. One file per responsibility:
//! - `layout`: the windup folder (binary, `<name>.db`, `config.toml`) and path resolution
//! - `schema`: `PRAGMA user_version` and ordered, transactional forward migrations
//! - `events`: the append-only event log (whole, then compressed batches, then summaries)
//! - `snapshot`: checked online-backup snapshots and the off-device hook
//! - `sync`: the generic sync-target trait (per-record results, batch capable)
//! - `wire` + `sync_socket`: the length-prefixed JSON protocol over a Unix socket, and its client
//! - `migrate`: the explicit move from `~/.config/windups/<name>/` into `~/windups/<name>/`
//! - `relocate`: config paths into a migrated folder (rewritten in the new copy only)
//! - `install`: first run makes `~/windups/<name>/` and the `~/.local/bin/<name>` link
//! - `audit`: read-only checks of any windup's DB (for Walter, the auditor)
//! - `commands`: the universal commands and their dispatcher; `talk`: plain words to a command
//! - `personality`: the one-trait personality and its prompt line
//! - `config`: the `[core]` config table and showing a config with secrets hidden
pub mod audit;
pub mod commands;
pub mod config;
pub mod events;
pub mod install;
pub mod layout;
pub mod migrate;
pub mod personality;
pub mod relocate;
pub mod schema;
pub mod snapshot;
pub mod sync;
#[cfg(unix)]
pub mod sync_socket;
pub mod talk;
pub mod wire;

/// The core's tables, in migration order. A windup appends these to its own list once (and
/// any later core migrations after them), so every windup DB carries the same core tables.
pub const CORE_V1: &[&str] = &[events::SCHEMA_V1, snapshot::SCHEMA_V1];
