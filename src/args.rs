//! These structs provide the CLI interface for the tiller CLI.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// A tool for manipulating financial data.
#[derive(Debug, Parser)]
pub struct Args {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    Import(Box<Import>),
}

/// Arguments common to all subcommands.
#[derive(Debug, Parser)]
pub struct Common {
    /// The directory where tiller data and configuration is held. Defaults to ~/.tiller
    #[arg(long, env = "TILLER_HOME")]
    home: Option<PathBuf>,
}

/// Import data from a file.
#[derive(Debug, Parser)]
pub(crate) struct Import {
    /// The file to import data from.
    #[arg(long)]
    pub(crate) file: PathBuf,

    #[command(flatten)]
    pub(crate) common: Common,
}
