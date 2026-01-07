use super::{FormulasMode, Out};
use crate::api::{sheet, tiller, Mode, Tiller};
use crate::backup::{SYNC_DOWN, SYNC_UP_PRE};
use crate::error::{ErrorType, IntoResult};
use crate::{Config, Result};
use anyhow::anyhow;
use tracing::{debug, info, warn};

/// Gets data from the tiller Google sheet and persists it to the local datastore. Returns an info
/// message that can be printed for the user.
pub async fn sync_down(config: Config, mode: Mode) -> Result<Out<()>> {
    // Backup SQLite database before modifying
    let sqlite_backup = config
        .backup()
        .copy_sqlite()
        .await
        .pub_result(ErrorType::Internal)?;
    debug!("Saved SQLite backup to {}", sqlite_backup.display());

    // Download data from Google Sheets (or test data in test mode)
    let sheet_client = sheet(config.clone(), mode).await?;
    let mut tiller_client = tiller(sheet_client).await.pub_result(ErrorType::Internal)?;
    let tiller_data = tiller_client.get_data().await.pub_result(ErrorType::Sync)?;

    // Save JSON backup of downloaded data
    let json_backup = config
        .backup()
        .save_json(SYNC_DOWN, &tiller_data)
        .await
        .pub_result(ErrorType::Internal)?;
    debug!("Saved JSON backup to {}", json_backup.display());

    // Save to SQLite database
    config
        .db()
        .save_tiller_data(&tiller_data)
        .await
        .pub_result(ErrorType::Database)?;

    Ok(Out::new_message(format!(
        "Synced {} transactions, {} categories, {} autocat rules from sheet to local datastore",
        tiller_data.transactions.data().len(),
        tiller_data.categories.data().len(),
        tiller_data.auto_cats.data().len()
    )))
}

/// Sends data from the local datastore to the Google sheet, returns a message that can be printed
/// for the user.
pub async fn sync_up(
    config: Config,
    mode: Mode,
    force: bool,
    formulas_mode: FormulasMode,
) -> Result<Out<()>> {
    // Precondition: verify database has transactions
    if config
        .db()
        .count_transactions()
        .await
        .pub_result(ErrorType::Database)?
        == 0
    {
        return Err(anyhow!(
            "Database has no transactions. Run 'tiller sync down' first to get data"
        ))
        .pub_result(ErrorType::Sync);
    }

    // Download current sheet state (or test data in test mode)
    let sheet_client = sheet(config.clone(), mode).await?;
    let mut tiller_client = tiller(sheet_client).await.pub_result(ErrorType::Internal)?;
    let current_sheet = tiller_client.get_data().await.pub_result(ErrorType::Sync)?;

    // Save sync-up-pre backup (before any modifications)
    let pre_backup = config
        .backup()
        .save_json(SYNC_UP_PRE, &current_sheet)
        .await
        .pub_result(ErrorType::Internal)?;
    debug!("Saved pre-upload backup to {}", pre_backup.display());

    // Conflict detection: compare current sheet with last sync-down backup
    let last_sync_down = config
        .backup()
        .load_latest_json(SYNC_DOWN)
        .await
        .pub_result(ErrorType::Internal)?;
    match last_sync_down {
        None => {
            if !force {
                return Err(anyhow!(
                    "No sync-down backup found. Run 'tiller sync down' first, \
                     or use --force to proceed without conflict detection"
                ))
                .pub_result(ErrorType::Sync);
            }
            warn!("No sync-down backup found, skipping conflict detection (--force)");
        }
        Some(backup_data) => {
            // Compare current sheet with backup
            if current_sheet != backup_data {
                if !force {
                    return Err(anyhow!(
                        "Sheet has been modified since last sync down. \
                         Run 'tiller sync down' first to merge changes, \
                         or use --force to overwrite"
                    ))
                    .pub_result(ErrorType::Sync);
                }
                warn!("Sheet differs from last sync-down, proceeding anyway (--force)");
            }
        }
    }

    // Build output data from SQLite
    let db_data = config
        .db()
        .get_tiller_data()
        .await
        .pub_result(ErrorType::Database)?;

    // Formula safety checks
    match formulas_mode {
        FormulasMode::Unknown => {
            if db_data.has_formulas() {
                return Err(anyhow!(
                    "Formulas detected in database. Use `--formulas preserve` to write formulas \
                     back to their original positions, or `--formulas ignore` to skip formulas"
                ))
                .pub_result(ErrorType::Sync);
            }
        }
        FormulasMode::Preserve => {
            // Check for gaps in original_order (indicating deleted rows) across all sheets
            if db_data.has_original_order_gaps() {
                if !force {
                    return Err(anyhow!(
                        "Row deletions detected (gaps in original_order). Formula positions may \
                         be corrupted. Use --force to proceed anyway, or use --formulas ignore"
                    ))
                    .pub_result(ErrorType::Sync);
                }
                warn!("Gaps detected in original_order, proceeding anyway (--force)");
            }
        }
        FormulasMode::Ignore => {
            debug!("Not considering formulas due to '--formulas ignore'");
        }
    }

    // Backup SQLite database before uploading
    let sqlite_backup = config
        .backup()
        .copy_sqlite()
        .await
        .pub_result(ErrorType::Internal)?;
    debug!("Saved SQLite backup to {}", sqlite_backup.display());

    // Backup Google Sheet via Drive API
    let backup_name = format!(
        "tiller-backup-{}",
        chrono::Local::now().format("%Y-%m-%d-%H%M%S")
    );
    let backup_id = tiller_client
        .copy_spreadsheet(&backup_name)
        .await
        .pub_result(ErrorType::Sync)?;
    debug!(
        "Created Google Sheet backup '{}' (ID: {})",
        backup_name, backup_id
    );

    // Execute batch clear and write to Google Sheet
    tiller_client
        .clear_and_write_data(&db_data)
        .await
        .pub_result(ErrorType::Sync)?;

    // Verification - re-fetch row counts and compare
    let (txn_count, cat_count, ac_count) = tiller_client
        .verify_write(&db_data)
        .await
        .pub_result(ErrorType::Sync)?;

    info!(
        "Synced {} transactions, {} categories, {} autocat rules to sheet",
        txn_count, cat_count, ac_count
    );

    Ok(Out::new_message(format!(
        "Synced {txn_count} transactions, {cat_count} categories, {ac_count} autocat rules \
        from local datastore to sheet",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{SheetCall, TestSheet, MODE_ENV};
    use crate::args::DeleteTransactionsArgs;
    use crate::test::TestEnv;

    #[tokio::test]
    async fn test_sync_down_saves_to_database() {
        let env = TestEnv::new().await;
        let config = env.config();

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
        let tiller_data = config.db().get_tiller_data().await.unwrap();
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

    #[tokio::test]
    async fn test_sync_up_errors_when_database_is_empty() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Database exists but is empty (no sync_down has been run)
        // sync_up should error because there are no transactions
        let result = sync_up(config, Mode::Testing, false, FormulasMode::Ignore).await;

        assert!(
            result.is_err(),
            "sync_up should fail when database is empty"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.to_lowercase().contains("sync down"),
            "Error should instruct user to run 'sync down' first, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_sync_up_creates_pre_backup() {
        let env = TestEnv::new().await;
        let config = env.config();

        // First run sync_down to populate the database (precondition for sync_up)
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Run sync_up - should create sync-up-pre backup
        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        // Verify sync-up-pre.*.json backup was created
        let backup_files: Vec<_> = std::fs::read_dir(config.backups())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        assert!(
            backup_files
                .iter()
                .any(|f| f.starts_with("sync-up-pre.") && f.ends_with(".json")),
            "sync-up-pre backup should be created. Found: {:?}",
            backup_files
        );
    }

    #[tokio::test]
    async fn test_sync_up_errors_without_sync_down_backup_no_force() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Delete all sync-down.*.json backup files
        for entry in std::fs::read_dir(config.backups()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("sync-down.") && name.ends_with(".json") {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }

        // Run sync_up without --force - should error because no sync-down backup exists
        let result = sync_up(config, Mode::Testing, false, FormulasMode::Ignore).await;

        assert!(
            result.is_err(),
            "sync_up should fail when no sync-down backup exists and --force not provided"
        );
        let err_msg = result.unwrap_err().to_string().to_lowercase();
        assert!(
            err_msg.contains("sync-down") || err_msg.contains("backup"),
            "Error should mention missing sync-down backup, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_sync_up_proceeds_without_sync_down_backup_with_force() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Delete all sync-down.*.json backup files
        for entry in std::fs::read_dir(config.backups()).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("sync-down.") && name.ends_with(".json") {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }

        // Run sync_up WITH --force - should NOT error despite missing sync-down backup
        let result = sync_up(config, Mode::Testing, true, FormulasMode::Ignore).await;

        assert!(
            result.is_ok(),
            "sync_up should succeed with --force even without sync-down backup, got: {:?}",
            result.unwrap_err()
        );
    }

    #[tokio::test]
    async fn test_sync_up_errors_when_sheet_modified_no_force() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database and create backup
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Update the remote sheet with some change (row 1 is first data row, row 0 is header)
        let mut state = env.get_state();
        state
            .data
            .get_mut("Transactions")
            .unwrap()
            .get_mut(1)
            .unwrap()
            .get_mut(0)
            .unwrap()
            .push_str("Edit");
        env.set_state(state);

        // Run sync_up without --force - should error due to detected differences
        let result = sync_up(config, Mode::Testing, false, FormulasMode::Ignore).await;

        assert!(
            result.is_err(),
            "sync_up should fail when sheet differs from sync-down backup without --force"
        );
        let err_msg = result.unwrap_err().to_string().to_lowercase();
        assert!(
            err_msg.contains("modified")
                || err_msg.contains("conflict")
                || err_msg.contains("differ"),
            "Error should mention sheet modification/conflict, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_sync_up_proceeds_with_force_when_sheet_modified() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database and create backup
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Update the remote sheet with some change (row 1 is first data row, row 0 is header)
        let mut state = env.get_state();
        state
            .data
            .get_mut("Transactions")
            .unwrap()
            .get_mut(1)
            .unwrap()
            .get_mut(0)
            .unwrap()
            .push_str("Edit");
        env.set_state(state);

        // Run sync_up WITH --force - should succeed despite differences
        let result = sync_up(config, Mode::Testing, true, FormulasMode::Ignore).await;

        assert!(
            result.is_ok(),
            "sync_up should succeed with --force even when sheet was modified, got: {:?}",
            result.unwrap_err()
        );
    }

    #[tokio::test]
    async fn test_sync_up_errors_with_gaps_preserve_no_force() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Delete a transaction from the database to create a gap in original_order
        // (e.g., if we have rows with original_order 0, 1, 2, deleting row 1 creates gap 0, 2)
        let db = config.db();
        let data = db.get_tiller_data().await.unwrap();
        let txn_to_delete = &data.transactions.data()[1]; // Get second transaction
        let delete_args = DeleteTransactionsArgs::new(vec![&txn_to_delete.transaction_id]).unwrap();
        db.delete_transactions(delete_args).await.unwrap();

        // Run sync_up with --formulas preserve (no --force)
        // Should error because gaps detected and formulas would be misaligned
        let result = sync_up(config, Mode::Testing, false, FormulasMode::Preserve).await;

        assert!(
            result.is_err(),
            "sync_up should fail with gaps in original_order when --formulas preserve without --force"
        );
        let err_msg = result.unwrap_err().to_string().to_lowercase();
        assert!(
            err_msg.contains("gap") || err_msg.contains("deletion") || err_msg.contains("formula"),
            "Error should mention gaps/deletions/formulas, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_sync_up_proceeds_with_gaps_preserve_with_force() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Delete a transaction from the database to create a gap in original_order
        let db = config.db();
        let data = db.get_tiller_data().await.unwrap();
        let txn_to_delete = &data.transactions.data()[1];
        let delete_args = DeleteTransactionsArgs::new(vec![&txn_to_delete.transaction_id]).unwrap();
        db.delete_transactions(delete_args).await.unwrap();

        // Run sync_up with --formulas preserve AND --force
        // Should succeed despite gaps
        let result = sync_up(config, Mode::Testing, true, FormulasMode::Preserve).await;

        assert!(
            result.is_ok(),
            "sync_up should succeed with --force even when gaps detected, got: {:?}",
            result.unwrap_err()
        );
    }

    #[tokio::test]
    async fn test_sync_up_ignores_gaps_with_formulas_ignore() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Delete a transaction from the database to create a gap in original_order
        let db = config.db();
        let data = db.get_tiller_data().await.unwrap();
        let txn_to_delete = &data.transactions.data()[1];
        let delete_args = DeleteTransactionsArgs::new(vec![&txn_to_delete.transaction_id]).unwrap();
        db.delete_transactions(delete_args).await.unwrap();

        // Run sync_up with --formulas ignore (no --force needed)
        // Should succeed because we're ignoring formulas, so gaps don't matter
        let result = sync_up(config, Mode::Testing, false, FormulasMode::Ignore).await;

        assert!(
            result.is_ok(),
            "sync_up should succeed with --formulas ignore even when gaps exist, got: {:?}",
            result.unwrap_err()
        );
    }

    #[tokio::test]
    async fn test_sync_up_creates_sqlite_backup() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Count existing SQLite backups (sync_down creates one)
        let backup_count_before: usize = std::fs::read_dir(config.backups())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("tiller.sqlite.")
            })
            .count();

        // Run sync_up
        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        // Count SQLite backups after sync_up
        let backup_count_after: usize = std::fs::read_dir(config.backups())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("tiller.sqlite.")
            })
            .count();

        assert!(
            backup_count_after > backup_count_before,
            "sync_up should create a SQLite backup. Before: {}, After: {}",
            backup_count_before,
            backup_count_after
        );
    }

    #[tokio::test]
    async fn test_sync_up_creates_google_sheet_backup() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Clear call history to isolate sync_up calls
        let test_sheet = TestSheet::new(config.spreadsheet_id());
        test_sheet.clear_history();

        // Run sync_up
        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        // Check that copy_spreadsheet was called
        let history = test_sheet.call_history();
        let copy_calls: Vec<_> = history
            .iter()
            .filter(|c| matches!(c, SheetCall::CopySpreadsheet { .. }))
            .collect();

        assert!(
            !copy_calls.is_empty(),
            "sync_up should create a Google Sheet backup via copy_spreadsheet. Call history: {:?}",
            history
        );
    }

    #[tokio::test]
    async fn test_sync_up_clears_and_writes_sheet_data() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Clear call history to isolate sync_up calls
        let test_sheet = TestSheet::new(config.spreadsheet_id());
        test_sheet.clear_history();

        // Run sync_up
        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        // Check that clear_ranges was called
        let history = test_sheet.call_history();
        let clear_calls: Vec<_> = history
            .iter()
            .filter(|c| matches!(c, SheetCall::ClearRanges { .. }))
            .collect();

        assert!(
            !clear_calls.is_empty(),
            "sync_up should clear sheet ranges before writing. Call history: {:?}",
            history
        );

        // Check that write_ranges was called
        let write_calls: Vec<_> = history
            .iter()
            .filter(|c| matches!(c, SheetCall::WriteRanges { .. }))
            .collect();

        assert!(
            !write_calls.is_empty(),
            "sync_up should write data to sheet ranges. Call history: {:?}",
            history
        );

        // Verify clear happens before write
        let clear_idx = history
            .iter()
            .position(|c| matches!(c, SheetCall::ClearRanges { .. }));
        let write_idx = history
            .iter()
            .position(|c| matches!(c, SheetCall::WriteRanges { .. }));

        assert!(
            clear_idx < write_idx,
            "clear_ranges should be called before write_ranges"
        );
    }

    #[tokio::test]
    async fn test_sync_up_verifies_write() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Clear call history to isolate sync_up calls
        let test_sheet = TestSheet::new(config.spreadsheet_id());
        test_sheet.clear_history();

        // Run sync_up
        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        // Check that verification occurred - Get calls should happen after WriteRanges
        let history = test_sheet.call_history();

        // Find the position of WriteRanges
        let write_idx = history
            .iter()
            .position(|c| matches!(c, SheetCall::WriteRanges { .. }));

        // Find Get calls after WriteRanges (for verification)
        let get_after_write: Vec<_> = history
            .iter()
            .enumerate()
            .filter(|(idx, c)| {
                matches!(c, SheetCall::Get { .. }) && write_idx.map_or(false, |w| *idx > w)
            })
            .collect();

        assert!(
            !get_after_write.is_empty(),
            "sync_up should verify write by fetching data after WriteRanges. Call history: {:?}",
            history
        );
    }

    #[tokio::test]
    async fn test_sync_up_errors_with_formulas_unknown_when_formulas_exist() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database (test data includes formulas)
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Verify that formulas actually exist in the database
        let db_data = config.db().get_tiller_data().await.unwrap();
        assert!(
            db_data.has_formulas(),
            "Test precondition: database should contain formulas after sync_down"
        );

        // Run sync_up with FormulasMode::Unknown - should error because formulas exist
        let result = sync_up(config, Mode::Testing, false, FormulasMode::Unknown).await;

        assert!(
            result.is_err(),
            "sync_up should fail with FormulasMode::Unknown when formulas exist in database"
        );
        let err_msg = result.unwrap_err().to_string().to_lowercase();
        assert!(
            err_msg.contains("formula")
                && (err_msg.contains("preserve") || err_msg.contains("ignore")),
            "Error should mention formulas and suggest --formulas preserve or ignore, got: {}",
            err_msg
        );
    }
}
