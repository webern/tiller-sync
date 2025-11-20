//! Command handlers for the tiller CLI.
//!
//! This module contains implementations for all CLI subcommands.

mod auth;
mod init;
mod sync;

// Re-export command handlers
pub use auth::{auth, auth_verify};
pub use init::init;
pub use sync::sync_down;
