//! Serialization and deserialization structures for Google API Key and OAuth credential files.
//! - `api_key.json`: OAuth 2.0 client credentials from Google Cloud Console
//! - `token.json`: Access and refresh tokens obtained through OAuth consent

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents the structure of the `api_key.json` file downloaded from Google Cloud Console.
///
/// This file contains OAuth 2.0 Desktop Application credentials. The standard format from Google
/// has an "installed" wrapper around the actual credentials.
///
/// Example:
/// ```json
/// {
///   "installed": {
///     "client_id": "YOUR_CLIENT_ID.apps.googleusercontent.com",
///     "client_secret": "YOUR_CLIENT_SECRET",
///     "redirect_uris": ["http://localhost:3030"],
///     "auth_uri": "https://accounts.google.com/o/oauth2/auth",
///     "token_uri": "https://oauth2.googleapis.com/token"
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ApiKeyFile {
    /// Wrapper containing the installed application credentials
    pub installed: InstalledCredentials,
}

/// The actual OAuth credentials nested within the `api_key.json` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InstalledCredentials {
    /// OAuth client ID
    pub(crate) client_id: String,

    /// OAuth client secret
    pub(crate) client_secret: String,

    /// List of valid redirect URIs for OAuth callbacks
    /// For this application, should contain "http://localhost:3030"
    pub(crate) redirect_uris: Vec<String>,

    /// Google's OAuth authorization endpoint
    pub(crate) auth_uri: String,

    /// Google's OAuth token endpoint
    pub(crate) token_uri: String,
}

/// Represents the structure of the `token.json` file containing OAuth tokens.
///
/// This file is generated after successful OAuth consent and contains both access and refresh
/// tokens. It is updated when tokens are refreshed.
///
/// Example:
/// ```json
/// {
///   "access_token": "ya29.a0AfH6SMBx...",
///   "refresh_token": "1//0gHZnXz9dD8...",
///   "token_type": "Bearer",
///   "expiry": "2025-11-11T12:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TokenFile {
    /// The access token used for API requests
    pub(crate) access_token: String,

    /// The refresh token used to obtain new access tokens
    pub(crate) refresh_token: String,

    /// Token type, typically "Bearer"
    pub(crate) token_type: String,

    /// When the access token expires
    pub(crate) expiry: DateTime<Utc>,
}
