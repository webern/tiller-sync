//! Authentication command handlers for OAuth flow.
//!
//! This module implements the CLI commands for:
//! - `tiller auth` - Initial OAuth consent flow
//! - `tiller auth --verify` - Verify and refresh authentication

use crate::api::TokenProvider;
use crate::{Config, Result};
use anyhow::{anyhow, Context};
use log::info;

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

/// Extracts the spreadsheet ID from a Google Sheets URL
///
/// # Arguments
/// * `url` - The Google Sheets URL (e.g., "https://docs.google.com/spreadsheets/d/SPREADSHEET_ID/...")
///
/// # Returns
/// The spreadsheet ID or an error if the URL format is invalid
fn _extract_spreadsheet_id(url: &str) -> Result<&str> {
    // URL format: https://docs.google.com/spreadsheets/d/SPREADSHEET_ID/...
    let parts: Vec<&str> = url.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "d" && i + 1 < parts.len() {
            return Ok(parts[i + 1]);
        }
    }
    Err(anyhow!(
        "Invalid Google Sheets URL format. Expected: https://docs.google.com/spreadsheets/d/SPREADSHEET_ID"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_spreadsheet_id() {
        let url = "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL/edit";
        let id = _extract_spreadsheet_id(url).unwrap();
        assert_eq!(id, "7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL");

        let url2 = "https://docs.google.com/spreadsheets/d/ABC123";
        let id2 = _extract_spreadsheet_id(url2).unwrap();
        assert_eq!(id2, "ABC123");

        let invalid = "https://example.com/invalid";
        assert!(_extract_spreadsheet_id(invalid).is_err());
    }
}
