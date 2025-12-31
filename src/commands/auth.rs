//! Authentication command handlers for OAuth flow.
//!
//! This module implements the CLI commands for:
//! - `tiller auth` - Initial OAuth consent flow
//! - `tiller auth --verify` - Verify and refresh authentication

use crate::api::TokenProvider;
use crate::{Config, Result};
use anyhow::Context;
use tracing::info;

/// Handles the `tiller auth` command - runs the OAuth consent flow
///
/// This is the ONLY command that should open a browser for OAuth authentication.
///
/// This guides the user through setting up Google Sheets authentication:
/// 1. Checks for client_secret.json (provides instructions if missing)
/// 2. Opens browser for OAuth consent
/// 3. Saves tokens to token.json with required scopes
///
/// # Arguments
/// * `config` - Reference to the Config struct
///
/// # Errors
/// Returns an error if OAuth flow fails or if client_secret.json is missing
pub async fn auth(config: &Config) -> Result<()> {
    let _ = TokenProvider::initialize(config.client_secret_path(), config.token_path()).await?;
    Ok(())
}

/// Handles the `tiller auth --verify` command - verifies authentication
///
/// This command NEVER opens a browser or triggers an interactive OAuth flow.
/// It only verifies that existing cached tokens are valid.
///
/// This command:
/// 1. Checks that credentials and tokens exist
/// 2. Verifies tokens have the correct scopes
/// 3. Makes a test API call to verify access
/// 4. Reports the results to the user
///
/// If the token is missing, invalid, or has the wrong scopes, this command will
/// fail with an error message telling the user to run `tiller auth`.
///
/// # Arguments
/// * `config` - Reference to the Config struct
///
/// # Errors
/// Returns an error if verification fails, credentials are missing, or tokens are invalid.
/// NEVER opens a browser - always returns an error instead.
pub async fn auth_verify(config: &Config) -> Result<()> {
    let mut token_provider = TokenProvider::load(config.client_secret_path(), config.token_path())
        .await
        .context(
            "Unable to use the existing tokens found in the token JSON file. \n\n\
            You should run 'tiller auth' (without the --verify flag).",
        )?;
    token_provider
        .refresh()
        .await
        .context("Unable to refresh the token")?;
    info!("Your OAuth token is valid!");
    Ok(())
}
