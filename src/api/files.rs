//! Serialization and deserialization structures for Google API Key and OAuth credential files.
//! - `api_key.json`: OAuth 2.0 client credentials from Google Cloud Console
//! - `token.json`: Access and refresh tokens obtained through OAuth consent

use crate::{utils, Result};
use anyhow::Context;
use chrono::{DateTime, Utc};
use google_sheets4::client::serde_with::__private__::{DeError, Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::path::Path;

const REDIRECT: &str = "http://localhost:3030";

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
    installed: InstalledCredentials,
}

impl ApiKeyFile {
    /// Loads the OAuth client credentials from api_key.json
    ///
    /// # Arguments
    /// * `path` - Path to the api_key.json file
    ///
    /// # Returns
    /// The parsed ApiKeyFile structure
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed
    pub async fn load(path: &Path) -> Result<ApiKeyFile> {
        utils::deserialize(path)
            .await
            .context("Unable to read ApiKeyFile")
    }

    pub(crate) fn redirect_uri(&self) -> &str {
        self.installed.redirect_uris.value()
    }
}

/// The actual OAuth credentials nested within the `api_key.json` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InstalledCredentials {
    /// OAuth client ID
    client_id: String,

    /// OAuth client secret
    client_secret: String,

    /// List of valid redirect URIs for OAuth callbacks
    /// For this application, should contain "http://localhost:3030"
    redirect_uris: RedirectUris,

    /// Google's OAuth authorization endpoint
    auth_uri: String,

    /// Google's OAuth token endpoint
    token_uri: String,
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
    access_token: String,

    /// The refresh token used to obtain new access tokens
    refresh_token: String,

    /// Token type, typically "Bearer"
    token_type: String,

    /// When the access token expires
    expiry: DateTime<Utc>,
}

impl TokenFile {
    /// Loads OAuth tokens from token.json
    ///
    /// # Arguments
    /// * `path` - Path to the token.json file
    ///
    /// # Returns
    /// The parsed TokenFile structure
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed
    pub(crate) async fn load(path: &Path) -> Result<TokenFile> {
        utils::deserialize(path)
            .await
            .context("Unable to read TokenFile")
    }

    pub(crate) fn expiry(&self) -> DateTime<Utc> {
        self.expiry
    }
}

#[derive(Debug, Default, Clone)]
struct RedirectUris(Vec<String>);

impl RedirectUris {
    fn value(&self) -> &str {
        REDIRECT
    }
}

impl Serialize for RedirectUris {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RedirectUris {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<String>::deserialize(deserializer)?;
        if !vec.iter().any(|s| is_valid_redirect(s)) {
            return Err(D::Error::custom(format!(
                "At least one of the redirects needs to be {REDIRECT}, but this was not found. \
                When creating the redirect URI for your Google API Key, you must include \
                '{REDIRECT}'"
            )));
        }
        Ok(RedirectUris(vec))
    }
}

fn is_valid_redirect(s: &str) -> bool {
    s == REDIRECT || s == "127.0.0.1:3030"
}

impl From<ApiKeyFile> for yup_oauth2::ApplicationSecret {
    fn from(value: ApiKeyFile) -> Self {
        yup_oauth2::ApplicationSecret {
            client_id: value.installed.client_id,
            client_secret: value.installed.client_secret,
            token_uri: value.installed.token_uri,
            auth_uri: value.installed.auth_uri,
            redirect_uris: vec![REDIRECT.to_string()],
            project_id: None,
            client_email: None,
            auth_provider_x509_cert_url: None,
            client_x509_cert_url: None,
        }
    }
}
