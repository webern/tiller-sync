//! Configuration file handling for Tiller.
//!
//! The configuration file is stored at `$TILLER_HOME/config.json` and contains settings for
//! the Tiller application including the Google Sheet URL, backup settings, and authentication
//! file paths.

use crate::backup::Backup;
use crate::db::Db;
use crate::{utils, Result};
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const APP_NAME: &str = "tiller";
const CONFIG_VERSION: u8 = 1;
const BACKUP_COPIES: u32 = 5;
const SECRETS: &str = ".secrets";
const BACKUPS: &str = ".backups";
const CLIENT_SECRET_JSON: &str = "client_secret.json";
const TOKEN_JSON: &str = "token.json";
const CONFIG_JSON: &str = "config.json";
const TILLER_SQLITE: &str = "tiller.sqlite";

/// The `Config` object represents the configuration of the app. You instantiate it by providing
/// the path to `$TILLER_HOME` and from there it loads `$TILLER_HOME/config.json`. It provides
/// paths to other items that are either configurable or are expected in a certain location within
/// the tiller home directory.
#[derive(Debug, Clone)]
pub struct Config {
    root: PathBuf,
    backups: PathBuf,
    secrets: PathBuf,
    config_path: PathBuf,
    config_file: ConfigFile,
    db: Db,
    spreadsheet_id: String,
    sqlite_path: PathBuf,
}

impl Config {
    /// Creates the data directory, its subdirectories and:
    /// - Creates an initial `config.json` file using `sheet_url` along with default settings
    /// - Moves `secret_file` into its default location in the data dir.
    ///
    /// # Arguments
    /// - `dir` - The directory that will be the root of data directory, e.g. `$HOME/tiller`
    /// - `secret_file` - The downloaded OAuth 2.0 client credentials JSON needed to start the Google
    ///   OAuth workflow. This will be moved from the `secret_file` path to its default location and
    ///   name in the data directory.
    /// - `sheet_url` - The URL of the Google Sheet where the Tiller financial data is stored.
    ///   e.g.https://docs.google.com/spreadsheets/d/1a7Km9FxQwRbPt82JvN4LzYpH5OcGnWsT6iDuE3VhMjX
    ///
    /// # Errors
    /// - Returns an error if any file operations fail.
    pub async fn create(
        dir: impl Into<PathBuf>,
        secret_file: &Path,
        sheet_url: &str,
    ) -> Result<Self> {
        // Create the directory if it does not exist
        let maybe_relative = dir.into();
        utils::make_dir(&maybe_relative)
            .await
            .context("Unable to create the tiller home directory")?;

        // Canonicalize the directory path
        let root = utils::canonicalize(&maybe_relative).await?;

        // Create the subdirectories
        let backups_dir = root.join(".backups");
        utils::make_dir(&backups_dir).await?;
        let secrets_dir = root.join(".secrets");
        utils::make_dir(&secrets_dir).await?;

        // Move the Google OAuth client credentials file to its default location in the data dir
        let secret_destination = secrets_dir.join(CLIENT_SECRET_JSON);
        utils::rename(secret_file, secret_destination).await?;
        let config_path = root.join(CONFIG_JSON);

        // Create and save an initial ConfigFile in the datastore
        let config_file = ConfigFile {
            app_name: APP_NAME.to_string(),
            config_version: CONFIG_VERSION,
            sheet_url: sheet_url.to_string(),
            backup_copies: BACKUP_COPIES,
            client_secret_path: None,
            token_path: None,
        };
        config_file.save(&config_path).await?;

        // Initialize the SQLite database
        let db_path = root.join(TILLER_SQLITE);
        let db = Db::init(&db_path)
            .await
            .context("Unable to create SQLite DB")?;

        // Extract the spreadsheet ID from the URL
        let spreadsheet_id = extract_spreadsheet_id(sheet_url)
            .context("Failed to extract spreadsheet ID from sheet URL")?
            .to_string();

        // Return a new `Config` object that represents a data directory that is ready to use
        Ok(Self {
            root,
            backups: backups_dir,
            secrets: secrets_dir,
            config_path,
            config_file,
            db,
            spreadsheet_id,
            sqlite_path: db_path,
        })
    }

    /// This will
    /// - validate that the  `tiller_home` exists and that the config file exists
    /// - load the config file
    /// - validate that the backups and secrets directories exist
    /// - return the loaded configuration object
    pub async fn load(tiller_home: impl Into<PathBuf>) -> Result<Self> {
        let maybe_relative = tiller_home.into();
        let root = utils::canonicalize(&maybe_relative).await?;

        // Validate that the home directory exists.
        let _ = utils::read_dir(&root)
            .await
            .context("Tiller Home is missing")?;

        let config_path = root.join("config.json");
        if !config_path.is_file() {
            bail!("The config file is missing '{}'", config_path.display())
        }
        let config_file = ConfigFile::load(&config_path).await?;

        // Extract the spreadsheet ID from the URL
        let spreadsheet_id = extract_spreadsheet_id(&config_file.sheet_url)
            .context("Failed to extract spreadsheet ID from sheet URL")?
            .to_string();

        // Load the SQLite database
        let db_path = root.join(TILLER_SQLITE);
        let db = Db::load(&db_path)
            .await
            .context("Unable to load SQLite DB")?;

        let config = Self {
            root: root.clone(),
            backups: root.join(BACKUPS),
            secrets: root.join(SECRETS),
            config_path,
            config_file,
            db,
            spreadsheet_id,
            sqlite_path: db_path,
        };
        if !config.backups.is_dir() {
            bail!(
                "The backups directory is missing '{}'",
                config.backups.display()
            )
        }
        if !config.secrets.is_dir() {
            bail!(
                "The secrets directory is missing '{}'",
                config.secrets.display()
            )
        }
        Ok(config)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub(crate) fn db(&self) -> &Db {
        &self.db
    }

    pub fn backups(&self) -> &Path {
        &self.backups
    }

    pub fn secrets(&self) -> &Path {
        &self.secrets
    }

    pub fn sheet_url(&self) -> &str {
        &self.config_file.sheet_url
    }

    pub fn spreadsheet_id(&self) -> &str {
        &self.spreadsheet_id
    }

    pub fn sqlite_path(&self) -> &Path {
        &self.sqlite_path
    }

    pub fn backup_copies(&self) -> u32 {
        self.config_file.backup_copies
    }

    /// Creates a new `Backup` instance for managing backup files.
    pub fn backup(&self) -> Backup {
        Backup::new(self)
    }

    /// Returns the stored `client_secret_path` if it is absolute, otherwise resolves the relative path.
    pub fn client_secret_path(&self) -> PathBuf {
        self.resolve_secrets_file_path(self.config_file.client_secret_path())
    }

    /// Returns the stored `token_path` if it is absolute, otherwise resolves the relative path.
    pub fn token_path(&self) -> PathBuf {
        self.resolve_secrets_file_path(self.config_file.token_path())
    }

    /// Checks if `p` is relative, and if so, resolves it. Returns it unchanged if it is absolute.
    fn resolve_secrets_file_path(&self, p: PathBuf) -> PathBuf {
        if p.is_absolute() {
            return p;
        }
        self.root.join(p)
    }
}

/// Represents the serialization and deserialization format of the configuration file.
///
/// Example configuration:
/// ```json
/// {
///   "app_name": "tiller",
///   "config_version": "v0.1.0",
///   "sheet_url": "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL",
///   "backup_copies": 5,
///   "client_secret_path": ".secrets/client_secret.json",
///   "token_path": ".secrets/token.json"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
struct ConfigFile {
    /// Application name, should always be "tiller"
    app_name: String,

    /// Configuration file version
    config_version: u8,

    /// URL to the Tiller Google Sheet
    sheet_url: String,

    /// Number of backup copies to keep
    backup_copies: u32,

    /// Path to the OAuth 2.0 client credentials file (optional, relative to config.json or absolute)
    /// Defaults to $TILLER_HOME/.secrets/client_secret.json if not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret_path: Option<PathBuf>,

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
            sheet_url: String::new(),
            backup_copies: 5,
            client_secret_path: None,
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

    #[cfg(test)]
    /// Creates a new ConfigFile with the specified settings.
    pub fn new(
        sheet_url: String,
        backup_copies: u32,
        client_secret_path: Option<PathBuf>,
        token_path: Option<PathBuf>,
    ) -> Self {
        Self {
            app_name: APP_NAME.to_string(),
            config_version: CONFIG_VERSION,
            sheet_url,
            backup_copies,
            client_secret_path,
            token_path,
        }
    }

    /// Gets the client secret path.
    ///
    /// If the path is relative, it should be interpreted as relative to the config.json file.
    /// If None, defaults to $TILLER_HOME/.secrets/client_secret.json
    pub fn client_secret_path(&self) -> PathBuf {
        self.client_secret_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(SECRETS).join(CLIENT_SECRET_JSON))
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
}

/// Extracts the spreadsheet ID from a Google Sheets URL
///
/// # Arguments
/// * `url` - The Google Sheets URL (e.g., "https://docs.google.com/spreadsheets/d/SPREADSHEET_ID/...")
///
/// # Returns
/// The spreadsheet ID or an error if the URL format is invalid. Returns an empty string if the URL is empty.
fn extract_spreadsheet_id(url: &str) -> Result<&str> {
    // Handle empty URL case
    if url.is_empty() {
        return Ok(url);
    }

    // URL format: https://docs.google.com/spreadsheets/d/SPREADSHEET_ID/...
    // or: https://docs.google.com/spreadsheets/d/SPREADSHEET_ID?foo=bar
    let parts: Vec<&str> = url.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "d" && i + 1 < parts.len() {
            // Extract the ID and remove any query parameters or fragments
            let id_part = parts[i + 1];
            let id = id_part
                .split('?')
                .next()
                .unwrap_or(id_part)
                .split('#')
                .next()
                .unwrap_or(id_part);
            return Ok(id);
        }
    }
    Err(anyhow::anyhow!(
        "Invalid Google Sheets URL format. Expected: https://docs.google.com/spreadsheets/d/SPREADSHEET_ID"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;
    use utils;

    #[tokio::test]
    async fn test_config_create() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let home_dir = dir.path().join("tiller_home");
        let secret_source_file = dir.path().join("x.txt");
        let secret_content = "12345";
        let sheet_url = "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL/edit";
        utils::write(&secret_source_file, secret_content)
            .await
            .unwrap();

        // Run the function under test:
        let config = Config::create(&home_dir, &secret_source_file, &sheet_url)
            .await
            .unwrap();

        // Check some values on the config object
        assert_eq!(sheet_url, config.sheet_url());
        assert_eq!(
            "7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL",
            config.spreadsheet_id()
        );

        // Check for some files in the directory
        let found_secret_content = utils::read(&config.client_secret_path()).await.unwrap();
        assert_eq!(secret_content, found_secret_content);

        assert!(config.backups().is_dir());
        assert!(config.secrets().is_dir());
    }

    #[tokio::test]
    async fn test_config() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let home_dir = dir.path().to_owned();
        let secret_file = dir.path().join("foo.json");
        utils::write(&secret_file, "{}").await.unwrap();
        let url = "https://example.com/spreadsheets/d/MySheetIDX";
        let config = Config::create(home_dir, &secret_file, &url).await.unwrap();
        assert!(utils::read_dir(config.backups()).await.is_ok());
        assert!(utils::read_dir(config.secrets()).await.is_ok());
        assert_eq!("MySheetIDX", config.spreadsheet_id());
    }

    #[test]
    fn test_config_file_new() {
        let config = ConfigFile::new(
            "https://docs.google.com/spreadsheets/d/test".to_string(),
            10,
            Some(PathBuf::from("custom/client_secret.json")),
            Some(PathBuf::from("custom/token.json")),
        );

        assert_eq!(
            config.sheet_url,
            "https://docs.google.com/spreadsheets/d/test"
        );
        assert_eq!(config.backup_copies, 10);
    }

    #[test]
    fn test_config_file_default() {
        let config = ConfigFile::default();
        assert_eq!(config.sheet_url, "");
        assert_eq!(config.backup_copies, 5);
        assert_eq!(
            config.client_secret_path(),
            PathBuf::from(SECRETS).join(CLIENT_SECRET_JSON)
        );
        assert_eq!(config.token_path(), PathBuf::from(SECRETS).join(TOKEN_JSON));
    }

    #[tokio::test]
    async fn test_config_file_save_and_load() {
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
    async fn test_config_file_load_with_minimal_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let json = r#"{
            "app_name": "tiller",
            "config_version": 1,
            "sheet_url": "https://docs.google.com/spreadsheets/d/minimal",
            "backup_copies": 3
        }"#;

        let mut file = tokio::fs::File::create(&config_path).await.unwrap();
        file.write_all(json.as_bytes()).await.unwrap();

        let config = ConfigFile::load(&config_path).await.unwrap();

        assert_eq!(
            config.sheet_url,
            "https://docs.google.com/spreadsheets/d/minimal"
        );
        assert_eq!(config.backup_copies, 3);
        assert_eq!(
            config.client_secret_path(),
            PathBuf::from(SECRETS).join(CLIENT_SECRET_JSON)
        );
        assert_eq!(config.token_path(), PathBuf::from(SECRETS).join(TOKEN_JSON));
    }

    #[tokio::test]
    async fn test_config_file_load_invalid_app_name() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let json = r#"{
            "app_name": "wrong_app",
            "config_version": 1,
            "sheet_url": "https://docs.google.com/spreadsheets/d/test",
            "backup_copies": 5
        }"#;

        let mut file = tokio::fs::File::create(&config_path).await.unwrap();
        file.write_all(json.as_bytes()).await.unwrap();

        let result = ConfigFile::load(&config_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid app_name"));
    }

    #[test]
    fn test_config_file_serialization_omits_none_fields() {
        let config = ConfigFile::new(
            "https://docs.google.com/spreadsheets/d/test".to_string(),
            5,
            None,
            None,
        );

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("client_secret_path"));
        assert!(!json.contains("token_path"));
    }

    #[tokio::test]
    async fn test_config_file_save_file() {
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

    #[test]
    fn test_extract_spreadsheet_id_1() {
        let url = "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL/edit";
        let id = extract_spreadsheet_id(url).unwrap();
        assert_eq!(id, "7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL");

        let url2 = "https://docs.google.com/spreadsheets/d/ABC123";
        let id2 = extract_spreadsheet_id(url2).unwrap();
        assert_eq!(id2, "ABC123");

        let invalid = "https://example.com/invalid";
        assert!(extract_spreadsheet_id(invalid).is_err());

        // Empty URL should return empty string
        let empty = "";
        let id_empty = extract_spreadsheet_id(empty).unwrap();
        assert_eq!(id_empty, "");
    }

    #[test]
    fn test_extract_spreadsheet_id_2() {
        let url = "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL?foo=bar";
        let id = extract_spreadsheet_id(url).unwrap();
        assert_eq!(id, "7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL");

        let url2 = "https://docs.google.com/spreadsheets/d/ABC123";
        let id2 = extract_spreadsheet_id(url2).unwrap();
        assert_eq!(id2, "ABC123");

        let invalid = "https://example.com/invalid";
        assert!(extract_spreadsheet_id(invalid).is_err());

        // Empty URL should return empty string
        let empty = "";
        let id_empty = extract_spreadsheet_id(empty).unwrap();
        assert_eq!(id_empty, "");
    }
}
