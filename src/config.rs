//! Configuration file handling for Tiller.
//!
//! The configuration file is stored at `$TILLER_HOME/config.json` and contains settings for
//! the Tiller application including the Google Sheet URL, backup settings, and authentication
//! file paths.

use crate::{utils, Home, Result};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const APP_NAME: &str = "tiller";
const CONFIG_VERSION: u8 = 1;
const SECRETS: &str = ".secrets";
const API_KEY_JSON: &str = "api_key.json";
const TOKEN_JSON: &str = "token.json";

/// Represents the serialization and deserialization format of the configuration file.
///
/// Example configuration:
/// ```json
/// {
///   "app_name": "tiller",
///   "config_version": "v0.1.0",
///   "tiller_sheet": "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL",
///   "backup_copies": 5,
///   "api_key_path": ".secrets/api_key.json",
///   "token_path": ".secrets/token.json"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ConfigFile {
    /// Application name, should always be "tiller"
    app_name: String,

    /// Configuration file version
    config_version: u8,

    /// URL to the Tiller Google Sheet
    tiller_sheet: String,

    /// Number of backup copies to keep
    backup_copies: u32,

    /// Path to the Google API key file (optional, relative to config.json or absolute)
    /// Defaults to $TILLER_HOME/.secrets/api_key.json if not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key_path: Option<PathBuf>,

    /// Path to the OAuth token file (optional, relative to config.json or absolute)
    /// Defaults to $TILLER_HOME/.secrets/token.json if not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    token_path: Option<PathBuf>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            app_name: APP_NAME.to_string(),
            config_version: CONFIG_VERSION,
            tiller_sheet: String::new(),
            backup_copies: 5,
            api_key_path: None,
            token_path: None,
        }
    }
}

impl ConfigFile {
    /// Loads a ConfigFile asynchronously from the specified path.
    ///
    /// # Arguments
    /// * `path` - Path to the config.json file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read config file at {}", path.display()))?;

        let config: ConfigFile = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config file at {}", path.display()))?;

        // Validate app_name
        anyhow::ensure!(
            config.app_name == APP_NAME,
            "Invalid app_name in config file: expected '{}', got '{}'",
            APP_NAME,
            config.app_name
        );

        Ok(config)
    }

    /// Saves the ConfigFile to the specified path.
    ///
    /// # Arguments
    /// * `path` - Path where the config.json file should be saved
    ///
    /// # Errors
    /// Returns an error if the file cannot be written
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let p = path.as_ref();
        let data = serde_json::to_string_pretty(self).context("Unable to serialize config")?;
        utils::write(p, data)
            .await
            .context("Unable to write config file")
    }

    /// Creates a new ConfigFile with the specified settings.
    pub fn new(
        tiller_sheet: String,
        backup_copies: u32,
        api_key_path: Option<PathBuf>,
        token_path: Option<PathBuf>,
    ) -> Self {
        Self {
            app_name: APP_NAME.to_string(),
            config_version: CONFIG_VERSION,
            tiller_sheet,
            backup_copies,
            api_key_path,
            token_path,
        }
    }

    /// Gets the Tiller Google Sheet URL.
    pub fn tiller_sheet(&self) -> &str {
        &self.tiller_sheet
    }

    /// Gets the number of backup copies to keep.
    pub fn backup_copies(&self) -> u32 {
        self.backup_copies
    }

    /// Gets the API key path.
    ///
    /// If the path is relative, it should be interpreted as relative to the config.json file.
    /// If None, defaults to $TILLER_HOME/.secrets/api_key.json
    pub fn api_key_path(&self) -> PathBuf {
        self.api_key_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(SECRETS).join(API_KEY_JSON))
    }

    /// Gets the token path.
    ///
    /// If the path is relative, it should be interpreted as relative to the config.json file.
    /// If None, defaults to $TILLER_HOME/.secrets/token.json
    pub fn token_path(&self) -> PathBuf {
        self.token_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(SECRETS).join(TOKEN_JSON))
    }

    /// Returns the stored `api_key_path` if it is absolute, otherwise resolves the relative path.
    pub fn resolve_api_key_path(&self, home: &Home) -> PathBuf {
        Self::resolve_secrets_file_path(self.api_key_path(), home)
    }

    /// Returns the stored `token_path` if it is absolute, otherwise resolves the relative path.
    pub fn resolve_token_path(&self, home: &Home) -> PathBuf {
        Self::resolve_secrets_file_path(self.token_path(), home)
    }

    /// Checks if `p` is relative, and if so, resolves it. Returns it unchanged if it is absolute.
    fn resolve_secrets_file_path(p: PathBuf, home: &Home) -> PathBuf {
        if p.is_absolute() {
            return p;
        }
        home.secrets_dir().join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn test_config_new() {
        let config = ConfigFile::new(
            "https://docs.google.com/spreadsheets/d/test".to_string(),
            10,
            Some(PathBuf::from("custom/api_key.json")),
            Some(PathBuf::from("custom/token.json")),
        );

        assert_eq!(
            config.tiller_sheet(),
            "https://docs.google.com/spreadsheets/d/test"
        );
        assert_eq!(config.backup_copies(), 10);
    }

    #[test]
    fn test_config_default() {
        let config = ConfigFile::default();
        assert_eq!(config.tiller_sheet(), "");
        assert_eq!(config.backup_copies(), 5);
        assert_eq!(
            config.api_key_path(),
            PathBuf::from(SECRETS).join(API_KEY_JSON)
        );
        assert_eq!(config.token_path(), PathBuf::from(SECRETS).join(TOKEN_JSON));
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let original_config = ConfigFile::new(
            "https://docs.google.com/spreadsheets/d/test123".to_string(),
            7,
            Some(PathBuf::from(".secrets/my_key.json")),
            Some(PathBuf::from(".secrets/my_token.json")),
        );

        // Save the config
        original_config.save(&config_path).await.unwrap();

        // Load it back
        let loaded_config = ConfigFile::load(&config_path).await.unwrap();

        assert_eq!(original_config, loaded_config);
    }

    #[tokio::test]
    async fn test_load_with_minimal_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let json = r#"{
            "app_name": "tiller",
            "config_version": 1,
            "tiller_sheet": "https://docs.google.com/spreadsheets/d/minimal",
            "backup_copies": 3
        }"#;

        let mut file = tokio::fs::File::create(&config_path).await.unwrap();
        file.write_all(json.as_bytes()).await.unwrap();

        let config = ConfigFile::load(&config_path).await.unwrap();

        assert_eq!(
            config.tiller_sheet(),
            "https://docs.google.com/spreadsheets/d/minimal"
        );
        assert_eq!(config.backup_copies(), 3);
        assert_eq!(
            config.api_key_path(),
            PathBuf::from(SECRETS).join(API_KEY_JSON)
        );
        assert_eq!(config.token_path(), PathBuf::from(SECRETS).join(TOKEN_JSON));
    }

    #[tokio::test]
    async fn test_load_invalid_app_name() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let json = r#"{
            "app_name": "wrong_app",
            "config_version": 1,
            "tiller_sheet": "https://docs.google.com/spreadsheets/d/test",
            "backup_copies": 5
        }"#;

        let mut file = tokio::fs::File::create(&config_path).await.unwrap();
        file.write_all(json.as_bytes()).await.unwrap();

        let result = ConfigFile::load(&config_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid app_name"));
    }

    #[test]
    fn test_serialization_omits_none_fields() {
        let config = ConfigFile::new(
            "https://docs.google.com/spreadsheets/d/test".to_string(),
            5,
            None,
            None,
        );

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("api_key_path"));
        assert!(!json.contains("token_path"));
    }

    #[tokio::test]
    async fn test_save_file() {
        let original = ConfigFile::new(
            "https://docs.google.com/spreadsheets/d/test".to_string(),
            5,
            None,
            None,
        );

        let t = TempDir::new().unwrap();
        let path = t.path().join("file.json");
        original.save(&path).await.unwrap();

        let read = ConfigFile::load(&path).await.unwrap();

        assert_eq!(original, read);
    }
}
