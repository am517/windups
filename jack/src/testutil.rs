//! Shared test helpers.
pub(crate) use crate::cli::Cli;
pub(crate) use crate::config::has_ext;
pub(crate) use crate::meta::Meta;
pub(crate) use crate::run::run;
pub(crate) use crate::{config, providers};
use clap::Parser;
pub(crate) use std::fs;
pub(crate) use std::path::Path;

pub(crate) fn cli(args: &[&str]) -> Cli {
    Cli::parse_from(std::iter::once("jack").chain(args.iter().copied()))
}
