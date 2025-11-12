//! Google Sheets API client creation and management.
//!
//! This module provides functionality to create authenticated Google Sheets API clients
//! using credentials from api_key.json and token.json files.

use crate::api::oauth;
use crate::Result;
use anyhow::Context;
use google_sheets4::hyper::client::HttpConnector;
use google_sheets4::hyper_rustls::HttpsConnector;
use google_sheets4::{hyper, hyper_rustls, Sheets};
use std::path::Path;
use yup_oauth2;

/// Creates an authenticated Google Sheets API client
///
/// This function:
/// 1. Resolves paths to api_key.json and token.json
/// 2. Loads OAuth credentials
/// 3. Checks if the token needs refreshing
/// 4. Creates an authenticated Sheets API client
///
/// # Arguments
/// * `home` - Reference to the Home struct for path resolution
/// * `api_key_path` - Optional custom path to api_key.json
/// * `token_path` - Optional custom path to token.json
///
/// # Returns
/// An authenticated Sheets API client ready to make requests
///
/// # Errors
/// Returns an error if credentials are missing, invalid, or if the client cannot be created
pub async fn create_sheets_client(
    api_key_path: &Path,
    token_path: &Path,
) -> Result<Sheets<HttpsConnector<HttpConnector>>> {
    log::debug!("Creating Google Sheets API client");

    // Load and potentially refresh token
    let token = oauth::refresh_token_if_needed(api_key_path, token_path).await?;

    log::debug!("Token loaded, expiry: {}", token.expiry());

    // Read the application secret for yup-oauth2
    let secret = yup_oauth2::read_application_secret(&api_key_path)
        .await
        .with_context(|| {
            format!(
                "Failed to read application secret from {}",
                api_key_path.display()
            )
        })?;

    // Create authenticator that will handle token refresh automatically
    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk(token_path)
    .build()
    .await
    .context("Failed to create authenticator")?;

    // Create HTTPS connector
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .context("Failed to load native TLS roots")?
        .https_or_http()
        .enable_http1()
        .build();

    // Create hyper client
    let client = hyper::Client::builder().build(https);

    // Create Sheets API client
    let sheets = Sheets::new(client, auth);

    log::debug!("Google Sheets API client created successfully");
    Ok(sheets)
}

/// Verifies that the Sheets API client is working by making a test API call
///
/// # Arguments
/// * `sheets` - Reference to the Sheets API client
/// * `spreadsheet_id` - The spreadsheet ID to test access with
///
/// # Returns
/// The title of the spreadsheet if successful
///
/// # Errors
/// Returns an error if the API call fails
pub async fn verify_client(
    sheets: &Sheets<HttpsConnector<HttpConnector>>,
    spreadsheet_id: &str,
) -> Result<String> {
    log::debug!("Verifying Sheets API access");

    let result = sheets
        .spreadsheets()
        .get(spreadsheet_id)
        .doit()
        .await
        .context("Failed to access spreadsheet")?;

    let title = result
        .1
        .properties
        .and_then(|p| p.title)
        .unwrap_or_else(|| "Unknown".to_string());

    log::info!("✓ Successfully accessed spreadsheet: {title}");
    Ok(title)
}
