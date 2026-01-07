//! Command handlers for the tiller CLI.
//!
//! This module contains implementations for all CLI subcommands.

mod auth;
mod init;
mod mcp;
mod sync;
mod update;

use serde::Serialize;
use std::fmt::Debug;
use tracing::{debug, info};

pub use auth::{auth, auth_verify};
pub use init::init;
pub use mcp::mcp;
pub use sync::{sync_down, sync_up};
pub use update::{update_transactions, Updates};

/// The output type for a command. This allows the command to return a consistent message and,
/// optionally, structured data to both the command line and MCP server interfaces.
#[derive(Debug, Clone, Serialize)]
pub struct Out<T>
where
    T: Serialize + Clone + Debug,
{
    /// A message that can be printed to the user regarding the outcome of the command execution.
    message: String,

    /// Any structured data that needs to be output from the call.
    structure: Option<T>,
}

impl<T, S> From<S> for Out<T>
where
    T: Debug + Clone + Serialize,
    S: Into<String>,
{
    fn from(value: S) -> Self {
        Out::new_message(value)
    }
}

/// Controls how formulas are handled during `sync up`.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
    clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum FormulasMode {
    /// Default: error if formulas exist, prompting user to choose preserve or ignore.
    #[default]
    Unknown,
    /// Preserve formulas by writing them back to their original cell positions.
    Preserve,
    /// Ignore all formulas; only write values.
    Ignore,
}

serde_plain::derive_display_from_serialize!(FormulasMode);
serde_plain::derive_fromstr_from_deserialize!(FormulasMode);

impl<T> Out<T>
where
    T: Serialize + Clone + Debug,
{
    /// Create a new `Out` object that has `Some(structure)`.
    pub fn new<S>(message: S, structure: T) -> Self
    where
        S: Into<String>,
    {
        Self {
            message: message.into(),
            structure: Some(structure),
        }
    }

    /// Create a new `Out` object that has `None` for `structure`.
    pub fn new_message<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            message: message.into(),
            structure: None,
        }
    }

    /// Get the `message`.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Get the structured data stored in `structure`.
    pub fn structure(&self) -> Option<&T> {
        self.structure.as_ref()
    }

    /// Print the message to `info!` and the structured data (if it exists) as JSON to `debug!`.
    pub fn print(&self) {
        info!("{}", self.message);
        if let Some(structure) = self.structure() {
            if let Ok(json) = serde_json::to_string_pretty(structure) {
                debug!("Command output:\n\n{json}\n\n");
            }
        }
    }
}
