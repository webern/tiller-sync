//! Serialization and deserialization structures for Google OAuth credential files.
//! - `client_secret.json`: OAuth 2.0 client credentials from Google Cloud Console

use crate::api::OAUTH_SCOPES;
use crate::{utils, Result};
use anyhow::{bail, Context};
use chrono::{DateTime, Utc};
use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

/// This redirect needs to be present in the OAuth credential file, or else OAuth will not work.
const REDIRECT: &str = "http://localhost";

/// Represents a file that we want to `Serialize`, `Deserialize`, and read from memory in-between
/// serializations and deserialization. Basically we are just holding the `path` and the `data`
/// here.
#[derive(Default, Debug, Clone)]
pub(super) struct File<F>
where
    F: Serialize + DeserializeOwned + Clone + Debug,
{
    path: PathBuf,
    data: F,
}

impl<F> File<F>
where
    F: Serialize + DeserializeOwned + Clone + Debug,
{
    /// Load data from a file and create a File instance
    pub(super) async fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let data: F = utils::deserialize(&path).await?;
        Ok(Self { path, data })
    }

    /// Create a File instance with the given path and data
    pub(super) fn new(path: impl Into<PathBuf>, data: F) -> Self {
        Self {
            path: path.into(),
            data,
        }
    }

    /// Save the current data to the file
    pub(super) async fn save(&self) -> Result<()> {
        let json =
            serde_json::to_string_pretty(&self.data).context("Failed to serialize data to JSON")?;
        utils::write(&self.path, json).await?;

        // Set restrictive permissions on Unix-like systems
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, Permissions::from_mode(0o600))
                .context("Failed to set file permissions")?;
        }

        Ok(())
    }

    /// Get a reference to the data
    pub(super) fn data(&self) -> &F {
        &self.data
    }

    /// Get a mutable reference to the data
    pub(super) fn data_mut(&mut self) -> &mut F {
        &mut self.data
    }

    /// Get the file path
    pub(super) fn _path(&self) -> &Path {
        &self.path
    }
}

/// Represents the structure of the `client_secret.json` file downloaded from Google Cloud Console.
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
///     "redirect_uris": ["http://localhost"],
///     "auth_uri": "https://accounts.google.com/o/oauth2/auth",
///     "token_uri": "https://oauth2.googleapis.com/token"
///   }
/// }
/// ```
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct SecretFile {
    /// Wrapper containing the installed application credentials
    installed: InstalledCredentials,
}

impl SecretFile {
    /// Loads the OAuth client credentials from client_secret.json
    ///
    /// # Arguments
    /// * `path` - Path to the client_secret.json file
    ///
    /// # Returns
    /// The parsed ClientSecretFile structure
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed
    pub(crate) async fn _load(path: &Path) -> Result<SecretFile> {
        utils::deserialize(path)
            .await
            .context("Unable to read ClientSecretFile")
    }

    /// Get the redirect URI
    // TODO: remove if it is not being used.
    pub(crate) fn _redirect_uri(&self) -> &str {
        self.installed.redirect_uris._value()
    }

    /// Get the client ID
    pub(super) fn client_id(&self) -> &str {
        &self.installed.client_id
    }

    /// Get the client secret
    pub(super) fn client_secret(&self) -> &str {
        &self.installed.client_secret
    }

    /// Get the auth URI
    pub(super) fn auth_uri(&self) -> &str {
        &self.installed.auth_uri
    }

    /// Get the token URI
    pub(super) fn token_uri(&self) -> &str {
        &self.installed.token_uri
    }
}

/// The actual OAuth credentials nested within the `client_secret.json` file.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct InstalledCredentials {
    /// OAuth client ID
    client_id: String,

    /// OAuth client secret
    client_secret: String,

    /// List of valid redirect URIs for OAuth callbacks
    /// For this application, should contain "http://localhost" (without a port number)
    redirect_uris: RedirectUris,

    /// Google's OAuth authorization endpoint
    auth_uri: String,

    /// Google's OAuth token endpoint
    token_uri: String,
}

#[derive(Default, Debug, Clone)]
struct RedirectUris(Vec<String>);

impl RedirectUris {
    fn _value(&self) -> &str {
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
    s == REDIRECT || s == "http://127.0.0.1"
}

/// This is how we save the token information that we receive from Google OAuth. We created our own
/// structure for this instead of saving Google's structure. We just wanted the structure to be a
/// bit more ergonomic.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct TokenFile {
    scopes: Vec<String>,
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
    id_token: Option<String>,
}

impl TokenFile {
    pub(super) async fn _load(p: impl AsRef<Path>) -> Result<Self> {
        let token_file: Self = utils::deserialize(p.as_ref())
            .await
            .context("Unable to deserialize the token JSON file")?;
        token_file._validate_scopes()?;
        Ok(token_file)
    }

    fn _validate_scopes(&self) -> Result<()> {
        let found_scopes: HashSet<&str> = self.scopes.iter().map(|s| s.as_str()).collect();
        for &required_scope in OAUTH_SCOPES {
            if !found_scopes.contains(required_scope) {
                bail!("OAuth scope '{required_scope}' is missing.");
            }
        }
        Ok(())
    }

    /// Create a new TokenFile
    pub(super) fn new(
        scopes: Vec<String>,
        access_token: String,
        refresh_token: String,
        expires_at: DateTime<Utc>,
        id_token: Option<String>,
    ) -> Self {
        Self {
            scopes,
            access_token,
            refresh_token,
            expires_at,
            id_token,
        }
    }

    /// Get the access token
    pub(super) fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Get the refresh token
    pub(super) fn refresh_token(&self) -> &str {
        &self.refresh_token
    }

    /// Check if the token is expired or will expire soon (within 5 minutes)
    pub(super) fn is_expired(&self) -> bool {
        let now = Utc::now();
        let buffer = chrono::Duration::minutes(5);
        self.expires_at <= now + buffer
    }

    /// Update the token with new values
    pub(super) fn update(
        &mut self,
        access_token: String,
        expires_at: DateTime<Utc>,
        refresh_token: Option<String>,
    ) {
        self.access_token = access_token;
        self.expires_at = expires_at;
        if let Some(rt) = refresh_token {
            self.refresh_token = rt;
        }
    }
}

#[tokio::test]
async fn test_client_secret_good_redirect() {
    use tempfile::TempDir;
    use utils;

    let json_data = String::from(
        r#"
{
    "installed": {
        "client_id": "YOUR_CLIENT_ID.apps.googleusercontent.com",
        "client_secret": "YOUR_CLIENT_SECRET",
        "redirect_uris": ["http://localhost", "https://example.com:4040/whatever"],
        "auth_uri": "https://accounts.google.com/o/oauth2/auth",
        "token_uri": "https://oauth2.googleapis.com/token"
    }
}
"#,
    );
    let temp_dir = TempDir::new().unwrap();
    let p = temp_dir.path().join("file.json");
    utils::write(&p, json_data).await.unwrap();
    let secret_file = SecretFile::_load(&p).await.unwrap();
    assert_eq!("http://localhost", secret_file._redirect_uri());
}

#[tokio::test]
async fn test_client_secret_good_redirect_2() {
    use tempfile::TempDir;
    use utils;

    let json_data = String::from(
        r#"
{
    "installed": {
        "client_id": "YOUR_CLIENT_ID.apps.googleusercontent.com",
        "client_secret": "YOUR_CLIENT_SECRET",
        "redirect_uris": ["http://127.0.0.1", "https://example.com:4040/whatever"],
        "auth_uri": "https://accounts.google.com/o/oauth2/auth",
        "token_uri": "https://oauth2.googleapis.com/token"
    }
}
"#,
    );
    let temp_dir = TempDir::new().unwrap();
    let p = temp_dir.path().join("file.json");
    utils::write(&p, json_data).await.unwrap();
    let secret_file = SecretFile::_load(&p).await.unwrap();
    assert_eq!("http://localhost", secret_file._redirect_uri());
}

#[tokio::test]
async fn test_client_secret_bad_redirect() {
    use tempfile::TempDir;
    use utils;

    let json_data = String::from(
        r#"
{
    "installed": {
        "client_id": "YOUR_CLIENT_ID.apps.googleusercontent.com",
        "client_secret": "YOUR_CLIENT_SECRET",
        "redirect_uris": ["http://localhost:9900", "https://example.com:4040/whatever"],
        "auth_uri": "https://accounts.google.com/o/oauth2/auth",
        "token_uri": "https://oauth2.googleapis.com/token"
    }
}
"#,
    );
    let temp_dir = TempDir::new().unwrap();
    let p = temp_dir.path().join("file.json");
    utils::write(&p, json_data).await.unwrap();
    let parse_result = SecretFile::_load(&p).await;
    assert!(parse_result.is_err());
    let parse_error = parse_result.err().unwrap();
    let parse_error_message = format!("{parse_error:?}");
    assert!(
        parse_error_message.contains("At least one of the redirects needs to be http://localhost")
    );
}

#[tokio::test]
async fn test_validate_token_file_bad() {
    use tempfile::TempDir;
    use utils;

    let json = String::from(
        r##"
        {
            "scopes": [
                "https://www.googleapis.com/auth/spreadsheets"
            ],
            "access_token":"abc12",
            "refresh_token":"xyz89",
            "expires_at":"2025-01-01T00:00:00Z",
            "id_token":null
        }
    "##,
    );
    let tmp = TempDir::new().unwrap();
    let json_path = tmp.path().join("file.json");
    utils::write(&json_path, &json).await.unwrap();

    let validation_result = TokenFile::_load(&json_path).await;
    assert!(validation_result.is_err());
    let error_message = validation_result.err().unwrap().to_string();
    assert!(error_message.contains("https://www.googleapis.com/auth/drive.readonly"));
}

#[tokio::test]
async fn test_validate_token_file_good() {
    use tempfile::TempDir;
    use utils;

    let json = String::from(
        r##"
        {
            "scopes": [
                "https://www.googleapis.com/auth/spreadsheets",
                "https://www.googleapis.com/auth/drive.readonly"
            ],
            "access_token":"abc12",
            "refresh_token":"xyz89",
            "expires_at":"2025-01-01T00:00:00Z",
            "id_token":null
        }
    "##,
    );

    let tmp = TempDir::new().unwrap();
    let json_path = tmp.path().join("file.json");
    utils::write(&json_path, &json).await.unwrap();

    let _ = TokenFile::_load(&json_path).await.unwrap();
}
