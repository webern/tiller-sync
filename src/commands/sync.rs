use crate::api::{sheet, tiller, Mode, Tiller};
use crate::backup::{SYNC_DOWN, SYNC_UP_PRE};
use crate::{Config, Result};
use anyhow::bail;
use log::{debug, info};

pub async fn sync_down(config: Config, mode: Mode) -> Result<()> {
    // Backup SQLite database before modifying
    let sqlite_backup = config.backup().copy_sqlite().await?;
    debug!("Saved SQLite backup to {}", sqlite_backup.display());

    // Download data from Google Sheets (or test data in test mode)
    let sheet_client = sheet(config.clone(), mode).await?;
    let mut tiller_client = tiller(sheet_client).await?;
    let tiller_data = tiller_client.get_data().await?;

    // Save JSON backup of downloaded data
    let json_backup = config.backup().save_json(SYNC_DOWN, &tiller_data).await?;
    debug!("Saved JSON backup to {}", json_backup.display());

    // Save to SQLite database
    config.db().save_tiller_data(&tiller_data).await?;

    info!(
        "Synced {} transactions, {} categories, {} autocat rules from sheet to database",
        tiller_data.transactions.data().len(),
        tiller_data.categories.data().len(),
        tiller_data.auto_cats.data().len()
    );

    Ok(())
}

pub async fn sync_up(config: Config, mode: Mode) -> Result<()> {
    // Precondition: verify database has transactions
    if config.db().count_transactions()? == 0 {
        bail!("Database has no transactions, run 'tiller sync down' to get data");
    }

    // Download current sheet state (or test data in test mode)
    let sheet_client = sheet(config.clone(), mode).await?;
    let mut tiller_client = tiller(sheet_client).await?;
    let current_sheet = tiller_client.get_data().await?;

    // TODO: Build output data from SQLite

    // TODO: Conflict detection (compare with last sync-down.*.json)

    // TODO: Backup SQLite database (ACTUALLY, is this necessary? Review this)
    // let sqlite_backup = config.backup().copy_sqlite().await?;
    // info!("Saved SQLite backup to {}", sqlite_backup.display());

    // Save sync-up-pre backup
    let pre_backup = config
        .backup()
        .save_json(SYNC_UP_PRE, &current_sheet)
        .await?;
    debug!("Saved pre-upload backup to {}", pre_backup.display());

    // TODO: Backup Google Sheet via Drive API (deferred - not part of Backup struct)

    // TODO: Execute batch clear and write to Google Sheet

    // TODO: Verification - re-fetch row counts

    todo!("sync_up implementation not complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::MODE_ENV;
    use crate::utils;
    use tempfile::TempDir;

    /// Creates a minimal client_secret.json for testing.
    fn dummy_client_secret_json() -> &'static str {
        r#"{
            "installed": {
                "client_id": "test-client-id",
                "client_secret": "test-secret",
                "redirect_uris": ["http://localhost"],
                "auth_uri": "https://accounts.google.com/o/oauth2/auth",
                "token_uri": "https://oauth2.googleapis.com/token"
            }
        }"#
    }

    #[tokio::test]
    async fn test_sync_down_saves_to_database() {
        // Create temp directory structure
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("tiller");
        let secret_path = temp_dir.path().join("client_secret.json");

        // Create a minimal client_secret.json
        utils::write(&secret_path, dummy_client_secret_json())
            .await
            .unwrap();

        // Create config (initializes database)
        let sheet_url = "https://docs.google.com/spreadsheets/d/abc123/edit";
        let config = Config::create(&root, &secret_path, sheet_url)
            .await
            .unwrap();

        // Run sync_down
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Verify SQLite backup was created
        let backup_files: Vec<_> = std::fs::read_dir(config.backups())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        assert!(
            backup_files.iter().any(|f| f.starts_with("tiller.sqlite.")),
            "SQLite backup should be created"
        );

        // Verify JSON backup was created
        assert!(
            backup_files
                .iter()
                .any(|f| f.starts_with("sync-down.") && f.ends_with(".json")),
            "JSON backup should be created"
        );

        // Verify data was saved to database
        // TestSheet::default() has 20 transactions, 5 categories, 3 autocat rules
        let tiller_data = config.db()._get_tiller_data().await.unwrap();
        assert_eq!(
            tiller_data.transactions.data().len(),
            20,
            "Should have 20 transactions from TestSheet"
        );
        assert_eq!(
            tiller_data.categories.data().len(),
            5,
            "Should have 5 categories from TestSheet"
        );
        assert_eq!(
            tiller_data.auto_cats.data().len(),
            3,
            "Should have 3 autocat rules from TestSheet"
        );

        // Clean up env var
        std::env::remove_var(MODE_ENV);
    }
}
