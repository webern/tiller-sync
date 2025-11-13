//! These structs provide the CLI interface for the tiller CLI.

use clap::{Parser, Subcommand};
use log::{error, LevelFilter};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// tiller: A command-line tool for manipulating financial data.
///
/// The purpose of this program is to download your financial transactions from a Tiller Google
/// sheet (see https://tiller.com) into a local datastore. There you can manipulate tham as you
/// wish and then sync your changes back to your Tiller sheet.
///
/// You will need set up a Google Docs API Key and OAuth for this. See the README at
/// https://github.com/webern/tiller-sync for documentation on how to set this up.
///
/// There is also a mode in which an AI agent, like Claude or Claude Code, can use this program
/// through the mcp subcommand.
#[derive(Debug, Parser, Clone)]
pub struct Args {
    #[clap(flatten)]
    common: Common,

    #[command(subcommand)]
    command: Command,
}

impl Args {
    pub fn new(common: Common, command: Command) -> Self {
        Self { common, command }
    }

    pub fn common(&self) -> &Common {
        &self.common
    }

    pub fn command(&self) -> &Command {
        &self.command
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Authenticate with Google Sheets via OAuth.
    Auth(AuthArgs),
    /// Upload or Download Transactions, Categories and AutoCat tabs to/from your Tiller Sheet.
    Sync(SyncArgs),
}

/// Arguments common to all subcommands.
#[derive(Debug, Parser, Clone)]
pub struct Common {
    /// The logging verbosity. One of, from least to most verbose:
    /// none, error, warn, info, debug, trace
    ///
    /// This can be overridden by RUST_LOG. See the env_logger crate for instructions.
    #[arg(long, default_value_t = LevelFilter::Info)]
    log_level: LevelFilter,

    /// The directory where tiller data and configuration is held. Defaults to ~/tiller
    #[arg(long, env = "TILLER_HOME", default_value_t = default_tiller_home())]
    tiller_home: DisplayPath,
}

impl Common {
    pub fn new(log_level: LevelFilter, tiller_home: PathBuf) -> Self {
        Self {
            log_level,
            tiller_home: tiller_home.into(),
        }
    }

    pub fn log_level(&self) -> LevelFilter {
        self.log_level
    }

    pub fn tiller_home(&self) -> &DisplayPath {
        &self.tiller_home
    }
}

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpDown {
    Up,
    #[default]
    Down,
}

serde_plain::derive_display_from_serialize!(UpDown);
serde_plain::derive_fromstr_from_deserialize!(UpDown);

/// Authenticate with Google Sheets via OAuth.
#[derive(Debug, Parser, Clone)]
pub struct AuthArgs {
    /// Verify and refresh authentication.
    #[arg(long)]
    verify: bool,
}

impl AuthArgs {
    pub fn new(verify: bool) -> Self {
        Self { verify }
    }

    pub fn verify(&self) -> bool {
        self.verify
    }
}

/// Upload or Download Transactions, Categories and AutoCat tabs to/from your Tiller Sheet.
#[derive(Debug, Parser, Clone)]
pub struct SyncArgs {
    /// The direction to sync: "up" or "down"
    direction: UpDown,

    /// The path to the Google API Key file, defaults to $TILLER_HOME/.secrets/api_key.json
    api_key: Option<PathBuf>,

    /// The path to the Google OAuth token file, defaults to $TILLER_HOME/.secrets/token.json
    oath_token: Option<PathBuf>,
}

impl SyncArgs {
    pub fn new(direction: UpDown, api_key: Option<PathBuf>, oath_token: Option<PathBuf>) -> Self {
        Self {
            direction,
            api_key,
            oath_token,
        }
    }

    pub fn direction(&self) -> UpDown {
        self.direction
    }

    pub fn api_key(&self) -> Option<&PathBuf> {
        self.api_key.as_ref()
    }

    pub fn oath_token(&self) -> Option<&PathBuf> {
        self.oath_token.as_ref()
    }
}

fn default_tiller_home() -> DisplayPath {
    DisplayPath(match dirs::home_dir() {
        Some(home) => home.join("tiller"),
        None => {
            error!(
                "There was an error when trying to get your home directory. You can get around \
                this by providing --tiller-home or TILLER_HOME instead of relying on the default \
                tiller home directory. If you continue using the program right now, you may have \
                problems!",
            );
            PathBuf::from("tiller")
        }
    })
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DisplayPath(PathBuf);

impl From<PathBuf> for DisplayPath {
    fn from(value: PathBuf) -> Self {
        DisplayPath(value)
    }
}

impl Deref for DisplayPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for DisplayPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Display for DisplayPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}

impl FromStr for DisplayPath {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(PathBuf::from(s)))
    }
}

impl DisplayPath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}
