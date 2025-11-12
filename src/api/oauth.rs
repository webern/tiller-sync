//! OAuth 2.0 authentication flow implementation for Google Sheets API.
//!
//! This module handles the complete OAuth workflow including:
//! - Loading OAuth credentials from api_key.json
//! - Managing access and refresh tokens in token.json
//! - Running the OAuth consent flow with a local callback server
//! - Automatic token refresh when expired

use crate::api::files::{ApiKeyFile, TokenFile};
use crate::Result;
use anyhow::Context;
use std::path::Path;
use yup_oauth2;

const OAUTH_SCOPES: &[&str] = &["https://www.googleapis.com/auth/spreadsheets"];
const OAUTH_CALLBACK_PORT: u16 = 3030;

/// Saves OAuth tokens to token.json with restrictive file permissions
///
/// # Arguments
/// * `path` - Path where token.json should be saved
/// * `token` - The TokenFile to save
///
/// # Errors
/// Returns an error if the file cannot be written or permissions cannot be set
pub async fn _save_token(path: &Path, token: &TokenFile) -> Result<()> {
    let content = serde_json::to_string_pretty(token).context("Failed to serialize token")?;

    tokio::fs::write(path, content)
        .await
        .with_context(|| format!("Failed to write token.json at {}", path.display()))?;

    // Set restrictive permissions (0600 on Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, permissions)
            .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    }

    log::info!("Tokens saved to {}", path.display());
    Ok(())
}

/// Runs the complete OAuth consent flow
///
/// This function:
/// 1. Loads OAuth credentials from api_key.json
/// 2. Starts a local HTTP server on localhost:3030
/// 3. Opens the user's browser to the Google consent page
/// 4. Waits for the OAuth callback with authorization code
/// 5. Exchanges the code for access and refresh tokens
/// 6. Saves tokens to token.json
///
/// # Arguments
/// * `home` - Reference to the Home struct for path resolution
/// * `api_key_path` - Optional custom path to api_key.json
/// * `token_path` - Optional custom path to token.json
///
/// # Errors
/// Returns an error if any step fails (missing files, network errors, timeout, etc.)
pub async fn run_oauth_flow(api_key: &Path, token: &Path) -> Result<()> {
    log::info!("Starting OAuth consent flow");

    // Load API key
    log::info!("Loading OAuth credentials from {}", api_key.display());
    let api_key = ApiKeyFile::load(api_key).await?;

    log::info!("Using redirect URI: {}", api_key.redirect_uri());

    // Create yup-oauth2 authenticator
    let secret: yup_oauth2::ApplicationSecret = api_key.into();

    // Use yup-oauth2's built-in installed flow with redirect to localhost:3030
    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::HTTPPortRedirect(OAUTH_CALLBACK_PORT),
    )
    .persist_tokens_to_disk(token)
    .build()
    .await
    .context("Failed to create authenticator")?;

    log::info!("Opening browser for authorization...");
    log::info!("Local callback server listening on http://localhost:{OAUTH_CALLBACK_PORT}",);
    log::info!("If the browser doesn't open automatically, you may need to visit the URL manually");

    // Get the token - this will open the browser and wait for the callback
    let scopes: Vec<&str> = OAUTH_SCOPES.to_vec();
    let _token = auth
        .token(&scopes)
        .await
        .context("Failed to obtain OAuth token")?;

    log::info!(" Authorization successful!");
    log::info!(" Tokens saved to: {}", token.display());

    Ok(())
}

/// Returns a valid access token, automatically refreshing if needed
///
/// This function leverages yup-oauth2's automatic token refresh capability.
/// The library checks if the token is expired (within 1 minute of expiration)
/// and silently refreshes it using the refresh_token without browser interaction.
///
/// # Arguments
/// * `home` - Reference to the Home struct
/// * `api_key_path` - Optional custom path to api_key.json
/// * `token_path` - Optional custom path to token.json
///
/// # Returns
/// A valid TokenFile (refreshed if necessary)
///
/// # Errors
/// Returns an error if authentication files are missing or token refresh fails
pub async fn refresh_token_if_needed(api_key_path: &Path, token_path: &Path) -> Result<TokenFile> {
    // Load API key
    let api_key = ApiKeyFile::load(api_key_path).await?;

    // Create yup-oauth2 authenticator with persisted tokens
    let secret: yup_oauth2::ApplicationSecret = api_key.into();

    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::HTTPPortRedirect(OAUTH_CALLBACK_PORT),
    )
    .persist_tokens_to_disk(token_path)
    .build()
    .await
    .context("Failed to create authenticator")?;

    // Request token - yup-oauth2 automatically refreshes if needed (within 1 min of expiry)
    // This uses the cached refresh_token and does NOT open a browser
    let scopes: Vec<&str> = OAUTH_SCOPES.to_vec();
    let _ = auth
        .token(&scopes)
        .await
        .context("Failed to get valid token")?;

    // Load the token from disk (may have been refreshed by yup-oauth2)
    let token = TokenFile::load(token_path).await?;
    log::debug!("Token valid until: {}", token.expiry());

    Ok(token)
}
