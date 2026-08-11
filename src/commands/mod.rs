//! Command handlers for the tiller CLI.
//!
//! This module contains implementations for all CLI subcommands.

mod auth;
mod delete;
mod init;
mod insert;
mod mcp;
pub mod query;
mod sync;
mod update;

use crate::error::{ErrorType, IntoResult};
use crate::Result;
use anyhow::Context;
use serde::Serialize;
use std::fmt::{Debug, Display};
use std::io::Write;
use tracing::{debug, info};

pub use auth::{auth, auth_verify};
pub use delete::{delete_autocats, delete_categories, delete_transactions};
pub use init::init;
pub use insert::{insert_autocat, insert_category, insert_transaction};
pub use mcp::mcp;
pub use query::{query, schema, ColumnInfo, ForeignKeyInfo, IndexInfo, Rows, Schema, TableInfo};
pub use sync::{sync_down, sync_up};
pub use update::{update_autocats, update_categories, update_transactions};

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
    ///
    /// This is for commands whose output is the act itself (syncing, inserting, deleting). Commands
    /// that exist to return data to the user should use [`Self::print_data`] instead so the data
    /// reaches `stdout`.
    pub fn print(&self) {
        info!("{}", self.message);
        if let Some(structure) = self.structure() {
            if let Ok(json) = serde_json::to_string_pretty(structure) {
                debug!("Command output:\n\n{json}\n\n");
            }
        }
    }
}

impl<T> Out<T>
where
    T: Serialize + Clone + Debug + Display,
{
    /// Print the message to `info!` (which goes to `stderr`) and the structured data to `stdout`.
    ///
    /// Query-style commands exist to hand data back to the caller, so the data has to go to
    /// `stdout` where it can be redirected or piped. Logging stays on `stderr`, which keeps
    /// `stdout` clean for the data itself.
    pub fn print_data(&self) -> Result<()> {
        let mut stdout = std::io::stdout().lock();
        self.write_data(&mut stdout)
    }

    /// The implementation behind [`Self::print_data`], parameterized over the writer so it can be
    /// tested without capturing the process's `stdout`.
    pub(crate) fn write_data(&self, writer: &mut impl Write) -> Result<()> {
        info!("{}", self.message);
        if let Some(structure) = self.structure() {
            writeln!(writer, "{structure}")
                .context("Unable to write command output")
                .pub_result(ErrorType::Internal)?;
        }
        Ok(())
    }
}
