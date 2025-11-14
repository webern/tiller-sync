//! Authentication command handlers for OAuth flow.
//!
//! This module implements the CLI commands for:
//! - `tiller auth` - Initial OAuth consent flow
//! - `tiller auth --verify` - Verify and refresh authentication

use crate::api;
use crate::{Config, Result};
use anyhow::{anyhow, Context};
use colored::Colorize;

/// Handles the `tiller auth` command - runs the OAuth consent flow
///
/// This guides the user through setting up Google Sheets authentication:
/// 1. Checks for api_key.json (provides instructions if missing)
/// 2. Opens browser for OAuth consent
/// 3. Saves tokens to token.json
///
/// # Arguments
/// * `home` - Reference to the Home struct
///
/// # Errors
/// Returns an error if OAuth flow fails or if api_key.json is missing
pub async fn handle_auth_command(config: &Config) -> Result<()> {
    eprintln!("{}", "Setting up Google Sheets authentication...".bold());
    eprintln!();

    let api_key = config.api_key_path();
    let token = config.token_path();

    if tokio::fs::metadata(&api_key).await.is_err() {
        // TODO: Think about this message and send to README instead?
        eprintln!("{}", "Step 1: Obtain OAuth credentials".yellow().bold());
        eprintln!("  OAuth credentials file not found at:");
        eprintln!("  {}", api_key.display().to_string().cyan());
        eprintln!();
        eprintln!("  To obtain OAuth credentials:");
        eprintln!("  1. Visit {}", "https://console.cloud.google.com/".cyan());
        eprintln!("  2. Create a new project or select an existing one");
        eprintln!("  3. Enable the Google Sheets API");
        eprintln!(
            "  4. Create {} credentials",
            "OAuth 2.0 Desktop Application".cyan()
        );
        eprintln!(
            "  5. Set redirect URI to: {}",
            "http://localhost:3030".cyan()
        );
        eprintln!("  6. Download the credentials JSON file");
        eprintln!("  7. Save it to: {}", api_key.display().to_string().cyan());
        eprintln!();
        return Err(anyhow!(
            "OAuth credentials file not found. Please follow the instructions above."
        ));
    }

    eprintln!("{}", "✓ Found OAuth credentials".green());
    eprintln!();

    eprintln!(
        "{}",
        "Step 2: Authorize tiller to access your Google Sheets".bold()
    );
    eprintln!("  Opening browser for authorization...");
    eprintln!();

    // Run OAuth flow
    api::run_oauth_flow(&api_key, &token)
        .await
        .context("OAuth flow failed")?;

    eprintln!();
    eprintln!("{}", "✓ Authentication setup complete!".green().bold());

    Ok(())
}

/// Handles the `tiller auth --verify` command - verifies and refreshes authentication
///
/// This command:
/// 1. Checks that credentials and tokens exist
/// 2. Refreshes tokens if needed
/// 3. Makes a test API call to verify access
/// 4. Reports the results to the user
///
/// # Arguments
/// * `home` - Reference to the Home struct
///
/// # Errors
/// Returns an error if verification fails or if credentials are missing
pub async fn handle_auth_verify(config: &Config) -> Result<()> {
    eprintln!("{}", "Verifying Google Sheets authentication...".bold());
    eprintln!();

    let api_key_path = config.api_key_path();
    let token_path = config.token_path();

    // Check if credential files exist
    if tokio::fs::metadata(&api_key_path).await.is_err() {
        return Err(anyhow!(
            "OAuth credentials not found at {}. Run 'tiller auth' first.",
            api_key_path.display()
        ));
    }

    if tokio::fs::metadata(&token_path).await.is_err() {
        return Err(anyhow!(
            "OAuth tokens not found at {}. Run 'tiller auth' first.",
            token_path.display()
        ));
    }

    eprintln!("{}", "✓ Found OAuth credentials and tokens".green());

    // Refresh token if needed
    let token = api::refresh_token_if_needed(&api_key_path, &token_path)
        .await
        .context("Failed to refresh token")?;

    eprintln!("{}", "✓ Token is valid".green());
    eprintln!("{}", "✓ Token is valid".green());
    eprintln!("  Expiry: {}", token.expiry().to_rfc3339());

    // Extract spreadsheet ID from the tiller_sheet URL
    let spreadsheet_id = extract_spreadsheet_id(config.tiller_sheet_url())
        .context("Invalid tiller_sheet URL in config.json")?;

    // Create Sheets client and verify access
    eprintln!();
    eprintln!("Testing API access...");

    let client = api::create_sheets_client(&api_key_path, &token_path)
        .await
        .context("Failed to create Sheets API client")?;

    let title = api::verify_client(&client, spreadsheet_id)
        .await
        .context("Failed to verify Sheets API access")?;

    eprintln!();
    eprintln!(
        "{}",
        "✓ Authentication verified successfully!".green().bold()
    );
    eprintln!("  Spreadsheet: {}", title.cyan());
    eprintln!("  Access: {}", "Read/Write".cyan());

    Ok(())
}

/// Extracts the spreadsheet ID from a Google Sheets URL
///
/// # Arguments
/// * `url` - The Google Sheets URL (e.g., "https://docs.google.com/spreadsheets/d/SPREADSHEET_ID/...")
///
/// # Returns
/// The spreadsheet ID or an error if the URL format is invalid
fn extract_spreadsheet_id(url: &str) -> Result<&str> {
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
        let id = extract_spreadsheet_id(url).unwrap();
        assert_eq!(id, "7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL");

        let url2 = "https://docs.google.com/spreadsheets/d/ABC123";
        let id2 = extract_spreadsheet_id(url2).unwrap();
        assert_eq!(id2, "ABC123");

        let invalid = "https://example.com/invalid";
        assert!(extract_spreadsheet_id(invalid).is_err());
    }
}
