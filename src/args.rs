/*!

These structs provide the CLI interface for the fin CLI.

!*/

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// A tool for manipulating financial data.
#[derive(Debug, Parser)]
pub struct Fin {
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
    /// The directory where fin data and configuration is held. Defaults to ~/.fin
    #[arg(long, env = "FIN_HOME")]
    home: Option<PathBuf>,
}

impl Common {
    pub fn home(&self) -> Result<PathBuf> {
        match &self.home {
            Some(home) => Ok(home.to_owned()),
            None => Ok(home::home_dir()
                .context("Unable to find user home directory")?
                .join(".fin")),
        }
    }
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
