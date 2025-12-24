//! Command handlers for the tiller CLI.
//!
//! This module contains implementations for all CLI subcommands.

mod auth;
mod init;
mod sync;

// Re-export command handlers
pub use auth::{auth, auth_verify};
pub use init::init;
pub use sync::{sync_down, sync_up};

/// Controls how formulas are handled during `sync up`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormulasMode {
    /// Default: error if formulas exist, prompting user to choose preserve or ignore.
    #[default]
    Unknown,
    /// Preserve formulas by writing them back to their original cell positions.
    Preserve,
    /// Ignore all formulas; only write values.
    Ignore,
}
