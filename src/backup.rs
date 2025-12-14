//! Backup management for local file backups during sync operations.

use crate::model::TillerData;
use crate::{utils, Config, Result};
use anyhow::Context;
use chrono::Local;
use std::path::PathBuf;

/// Prefix for sync-down backup files.
pub const SYNC_DOWN: &str = "sync-down";

/// Prefix for sync-up-pre backup files (snapshot before upload).
pub const SYNC_UP_PRE: &str = "sync-up-pre";

/// Prefix for SQLite backup files.
pub const SQLITE: &str = "tiller.sqlite";

/// Manages backup file creation and rotation.
///
/// The `Backup` struct is immutable and owns copies of the paths and settings it needs.
/// Create a new instance via `Config::backup()` or `Backup::new()`.
#[derive(Debug, Clone)]
pub struct Backup {
    backups_dir: PathBuf,
    backup_copies: u32,
    sqlite_path: PathBuf,
}

impl Backup {
    /// Creates a new `Backup` instance from a `Config`.
    pub fn new(config: &Config) -> Self {
        Self {
            backups_dir: config.backups().to_path_buf(),
            backup_copies: config.backup_copies(),
            sqlite_path: config.sqlite_path().to_path_buf(),
        }
    }

    /// Saves `TillerData` as a pretty-printed JSON backup file.
    ///
    /// The filename format is `{prefix}.YYYY-MM-DD-NNN.json` where NNN is a sequence number.
    /// Automatically rotates old backups, keeping only `backup_copies` files.
    ///
    /// Returns the path to the created backup file.
    pub async fn save_json(&self, prefix: &str, data: &TillerData) -> Result<PathBuf> {
        let date = today();
        let seq = self.next_sequence_number(prefix, &date, "json").await?;
        let filename = format!("{prefix}.{date}-{seq:03}.json");
        let path = self.backups_dir.join(&filename);

        let json =
            serde_json::to_string_pretty(data).context("Failed to serialize TillerData to JSON")?;
        utils::write(&path, json).await?;

        self.rotate(prefix, "json").await?;

        Ok(path)
    }

    /// Copies the SQLite database file to the backups directory.
    ///
    /// The filename format is `tiller.sqlite.YYYY-MM-DD-NNN`.
    /// Automatically rotates old backups, keeping only `backup_copies` files.
    ///
    /// Returns the path to the created backup file.
    pub async fn copy_sqlite(&self) -> Result<PathBuf> {
        let date = today();
        let seq = self.next_sequence_number(SQLITE, &date, "").await?;
        let filename = format!("{SQLITE}.{date}-{seq:03}");
        let path = self.backups_dir.join(&filename);

        utils::copy(&self.sqlite_path, &path).await?;

        self.rotate(SQLITE, "").await?;

        Ok(path)
    }

    /// Scans the backups directory for existing files with the given prefix and date,
    /// and returns the next sequence number.
    async fn next_sequence_number(&self, prefix: &str, date: &str, extension: &str) -> Result<u32> {
        let pattern_start = format!("{prefix}.{date}-");
        let mut max_seq: u32 = 0;

        let mut dir = utils::read_dir(&self.backups_dir).await?;
        while let Some(entry) = dir
            .next_entry()
            .await
            .context("Failed to read directory entry")?
        {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            if name.starts_with(&pattern_start) {
                if let Some(seq) = parse_sequence_number(&name, prefix, date, extension) {
                    max_seq = max_seq.max(seq);
                }
            }
        }

        Ok(max_seq + 1)
    }

    /// Rotates old backup files, keeping only `backup_copies` files with the given prefix.
    async fn rotate(&self, prefix: &str, extension: &str) -> Result<()> {
        // Collect all matching backup files
        let mut files: Vec<(PathBuf, String)> = Vec::new();

        let mut dir = utils::read_dir(&self.backups_dir).await?;
        while let Some(entry) = dir
            .next_entry()
            .await
            .context("Failed to read directory entry")?
        {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_string();

            if is_backup_file(&name, prefix, extension) {
                files.push((entry.path(), name));
            }
        }

        // Sort by filename (which sorts by date and sequence number due to format)
        files.sort_by(|a, b| a.1.cmp(&b.1));

        // Delete oldest files if we have more than backup_copies
        let to_delete = files.len().saturating_sub(self.backup_copies as usize);
        for (path, _) in files.into_iter().take(to_delete) {
            utils::remove(&path).await?;
        }

        Ok(())
    }
}

/// Returns today's date in YYYY-MM-DD format.
fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Parses the sequence number from a backup filename.
/// Returns None if the filename doesn't match the expected pattern.
fn parse_sequence_number(filename: &str, prefix: &str, date: &str, extension: &str) -> Option<u32> {
    // Pattern: {prefix}.{date}-{NNN}.{ext} or {prefix}.{date}-{NNN} (no extension)
    let expected_start = format!("{prefix}.{date}-");

    if !filename.starts_with(&expected_start) {
        return None;
    }

    let remainder = &filename[expected_start.len()..];

    // Extract the sequence number part
    let seq_str = if extension.is_empty() {
        remainder
    } else {
        let expected_suffix = format!(".{extension}");
        remainder.strip_suffix(&expected_suffix)?
    };

    seq_str.parse().ok()
}

/// Checks if a filename is a backup file with the given prefix and extension.
fn is_backup_file(filename: &str, prefix: &str, extension: &str) -> bool {
    let starts_ok = filename.starts_with(&format!("{prefix}."));

    let ends_ok = if extension.is_empty() {
        // For SQLite backups, ensure it doesn't end with a known extension
        !filename.ends_with(".json")
    } else {
        filename.ends_with(&format!(".{extension}"))
    };

    starts_ok && ends_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sequence_number() {
        assert_eq!(
            parse_sequence_number(
                "sync-down.2025-12-14-001.json",
                "sync-down",
                "2025-12-14",
                "json"
            ),
            Some(1)
        );
        assert_eq!(
            parse_sequence_number(
                "sync-down.2025-12-14-042.json",
                "sync-down",
                "2025-12-14",
                "json"
            ),
            Some(42)
        );
        assert_eq!(
            parse_sequence_number(
                "tiller.sqlite.2025-12-14-003",
                "tiller.sqlite",
                "2025-12-14",
                ""
            ),
            Some(3)
        );
        // Wrong prefix
        assert_eq!(
            parse_sequence_number(
                "sync-up-pre.2025-12-14-001.json",
                "sync-down",
                "2025-12-14",
                "json"
            ),
            None
        );
        // Wrong date
        assert_eq!(
            parse_sequence_number(
                "sync-down.2025-12-13-001.json",
                "sync-down",
                "2025-12-14",
                "json"
            ),
            None
        );
    }

    #[test]
    fn test_is_backup_file() {
        assert!(is_backup_file(
            "sync-down.2025-12-14-001.json",
            "sync-down",
            "json"
        ));
        assert!(is_backup_file(
            "sync-up-pre.2025-12-14-001.json",
            "sync-up-pre",
            "json"
        ));
        assert!(is_backup_file(
            "tiller.sqlite.2025-12-14-001",
            "tiller.sqlite",
            ""
        ));
        assert!(!is_backup_file(
            "sync-down.2025-12-14-001.json",
            "sync-up-pre",
            "json"
        ));
        assert!(!is_backup_file(
            "tiller.sqlite.2025-12-14-001.json",
            "tiller.sqlite",
            ""
        ));
    }
}
