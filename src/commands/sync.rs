use super::{FormulasMode, Out};
use crate::api::{sheet, tiller, Mode, SyncIds, Tiller};
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

    // The sync IDs already in use locally, so that an identifier minted for a new sheet row
    // cannot collide with one this datastore already holds.
    let known_sync_ids = config
        .db()
        .sync_ids()
        .await
        .pub_result(ErrorType::Database)?;

    // Download data from Google Sheets (or test data in test mode). The sync ID assignment phase
    // runs as part of this fetch: any row of the Transactions tab that lacks an identifier is
    // given one and the sheet is stamped before the data comes back. The write is verified before
    // anything below commits to SQLite, so a failure here leaves nothing half-done locally.
    let sheet_client = sheet(config.clone(), mode).await?;
    let mut tiller_client = tiller(sheet_client).await.pub_result(ErrorType::Internal)?;
    let tiller_data = tiller_client
        .get_data(SyncIds::Assign(&known_sync_ids))
        .await
        .pub_result(ErrorType::Sync)?;

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
    let current_sheet = tiller_client
        .get_data(SyncIds::Read)
        .await
        .pub_result(ErrorType::Sync)?;

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
    let preserve_formulas = matches!(formulas_mode, FormulasMode::Preserve);
    let formulas_written = tiller_client
        .clear_and_write_data(&db_data, preserve_formulas)
        .await
        .pub_result(ErrorType::Sync)?;

    // Verification - re-fetch and compare against what we meant to write
    let counts = tiller_client
        .verify_write(&db_data)
        .await
        .pub_result(ErrorType::Sync)?;

    // Preserve formulas if requested.
    let formula_summary = if preserve_formulas {
        if counts.formulas != formulas_written {
            warn!(
                "Wrote {formulas_written} formulas but the sheet reports {}. A formula whose \
                 result is identical to its own text cannot be told apart from a plain value, so a \
                 small difference may be harmless. If formulas were lost, the pre-upload snapshot \
                 at {} holds the previous contents of the sheet.",
                counts.formulas,
                pre_backup.display()
            );
        }
        format!(", {formulas_written} formulas")
    } else {
        String::new()
    };

    info!(
        "Synced {} transactions, {} categories, {} autocat rules{formula_summary} to sheet",
        counts.transactions, counts.categories, counts.auto_cats
    );

    Ok(Out::new_message(format!(
        "Synced {} transactions, {} categories, {} autocat rules{formula_summary} \
        from local datastore to sheet",
        counts.transactions, counts.categories, counts.auto_cats
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        SheetCall, TestSheet, TestSheetState, AUTO_CAT, CATEGORIES, MODE_ENV, TRANSACTIONS,
    };
    use crate::args::DeleteTransactionsArgs;
    use crate::model::Transaction;
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
            6,
            "Should have 6 categories from TestSheet"
        );

        // Assert that "Hidden Category" has the value "Hide".
        // Bug fix: https://github.com/webern/tiller-sync/issues/24
        let hidden_category = tiller_data
            .categories
            .data()
            .iter()
            .find(|&item| item.category == "Hidden Category")
            .unwrap();
        assert_eq!(
            hidden_category.hide_from_reports, "Hide",
            "The 'Hide From Reports' column should be properly serialized and deserialized but it \
            wasn't. This could be a regression of https://github.com/webern/tiller-sync/issues/24"
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
        let delete_args = DeleteTransactionsArgs::new(vec![&txn_to_delete.sync_id]).unwrap();
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
        let delete_args = DeleteTransactionsArgs::new(vec![&txn_to_delete.sync_id]).unwrap();
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
        let delete_args = DeleteTransactionsArgs::new(vec![&txn_to_delete.sync_id]).unwrap();
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

        // Verify dates are written in US format (M/D/YYYY), not ISO format
        if let Some(SheetCall::WriteRanges { ranges }) = write_calls.first() {
            // Find the Transactions sheet data
            if let Some((_, rows)) = ranges.iter().find(|(r, _)| r.contains("Transactions")) {
                // Row 0 is header, row 1 is first data row. Column 1 is "Date"
                let date_value = &rows[1][1];
                assert!(
                    date_value.contains('/'),
                    "Date should be in US format (M/D/YYYY), got: {date_value}"
                );
            }
        }

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

    /// `--formulas preserve` now actually preserves formulas.
    ///
    /// See https://github.com/webern/tiller-sync/issues/35
    #[tokio::test]
    async fn test_sync_up_preserve_writes_formulas_to_the_sheet() {
        let env = TestEnv::new().await;
        let config = env.config();

        sync_down(config.clone(), Mode::Testing).await.unwrap();

        let db_data = config.db().get_tiller_data().await.unwrap();
        let expected_formulas = db_data.transactions.formulas().clone();
        assert!(
            !expected_formulas.is_empty(),
            "test precondition: the seed data should contain formulas"
        );

        let test_sheet = TestSheet::new(config.spreadsheet_id());
        test_sheet.clear_history();

        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Preserve)
            .await
            .unwrap();

        // Inspect what was actually sent to the sheet.
        let history = test_sheet.call_history();
        let SheetCall::WriteRanges { ranges } = history
            .iter()
            .find(|c| matches!(c, SheetCall::WriteRanges { .. }))
            .expect("sync_up should write data")
        else {
            unreachable!("filtered on WriteRanges")
        };
        let (_, written) = ranges
            .iter()
            .find(|(r, _)| r.contains(TRANSACTIONS))
            .expect("sync_up should write the Transactions tab");

        for (row_col, formula) in &expected_formulas {
            // +1 for the header row that to_rows prepends.
            let cell = written
                .get(row_col.row() + 1)
                .and_then(|row| row.get(row_col.col()));
            assert_eq!(
                cell,
                Some(formula),
                "the formula at {row_col} should have been written back, not its computed value"
            );
        }

        // And confirm it round-trips: reading the sheet again must find the same formulas.
        let mut client = tiller(sheet(config.clone(), Mode::Testing).await.unwrap())
            .await
            .unwrap();
        let from_sheet = client.get_data(SyncIds::Read).await.unwrap();
        assert_eq!(
            from_sheet.transactions.formulas(),
            &expected_formulas,
            "formulas should still be live formulas after sync up"
        );
    }

    /// With `--formulas ignore` the sheet should receive plain values, as it always has.
    #[tokio::test]
    async fn test_sync_up_ignore_writes_values_not_formulas() {
        let env = TestEnv::new().await;
        let config = env.config();

        sync_down(config.clone(), Mode::Testing).await.unwrap();

        let test_sheet = TestSheet::new(config.spreadsheet_id());
        test_sheet.clear_history();

        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        let history = test_sheet.call_history();
        let SheetCall::WriteRanges { ranges } = history
            .iter()
            .find(|c| matches!(c, SheetCall::WriteRanges { .. }))
            .expect("sync_up should write data")
        else {
            unreachable!("filtered on WriteRanges")
        };

        for (_, rows) in ranges {
            for row in rows {
                for cell in row {
                    assert!(
                        !cell.starts_with('='),
                        "--formulas ignore should not write any formula, found: {cell}"
                    );
                }
            }
        }
    }

    /// The summary has to say how many formulas were written, so a silent loss cannot look like a
    /// clean run again.
    #[tokio::test]
    async fn test_sync_up_reports_formula_count() {
        let env = TestEnv::new().await;
        let config = env.config();

        sync_down(config.clone(), Mode::Testing).await.unwrap();
        let expected = config
            .db()
            .get_tiller_data()
            .await
            .unwrap()
            .transactions
            .formulas()
            .len();

        let out = sync_up(config.clone(), Mode::Testing, false, FormulasMode::Preserve)
            .await
            .unwrap();

        assert!(
            out.message().contains(&format!("{expected} formulas")),
            "the summary should report the formula count, got: {}",
            out.message()
        );
    }

    /// A formula whose row was deleted locally has nowhere to go. It must be dropped rather than
    /// landing on whichever row shifted up into its place.
    #[tokio::test]
    async fn test_sync_up_preserve_drops_formulas_for_deleted_rows() {
        let env = TestEnv::new().await;
        let config = env.config();

        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Delete the last transaction, so the formula bound to the final row has no row left.
        let db = config.db();
        let data = db.get_tiller_data().await.unwrap();
        let total_formulas = data.transactions.formulas().len();
        let last = data.transactions.data().last().unwrap();
        let delete_args = DeleteTransactionsArgs::new(vec![&last.sync_id]).unwrap();
        db.delete_transactions(delete_args).await.unwrap();

        // --force because deleting a row leaves a gap in original_order.
        let out = sync_up(config.clone(), Mode::Testing, true, FormulasMode::Preserve)
            .await
            .unwrap();

        assert!(
            out.message()
                .contains(&format!("{} formulas", total_formulas - 1)),
            "the formula for the deleted row should be dropped, got: {}",
            out.message()
        );
    }

    #[tokio::test]
    async fn test_sync_roundtrip_preserves_data() {
        let env = TestEnv::new().await;
        let config = env.config();

        // Run sync_down to populate the database (this also seeds the TestSheet)
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Capture original sheet state after sync_down (seed data is now loaded)
        let test_sheet = TestSheet::new(config.spreadsheet_id());
        let original_state = test_sheet.get_state();

        // Clear history and run sync_up
        test_sheet.clear_history();
        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        // Get the data that was written during sync_up
        let history = test_sheet.call_history();
        let write_call = history
            .iter()
            .find(|c| matches!(c, SheetCall::WriteRanges { .. }))
            .expect("sync_up should write data");

        if let SheetCall::WriteRanges { ranges } = write_call {
            // Compare Transactions data
            let (_, written_transactions) = ranges
                .iter()
                .find(|(r, _)| r.contains("Transactions"))
                .expect("Should write Transactions");
            let original_transactions = original_state
                .data
                .get("Transactions")
                .expect("Original should have Transactions");

            // Compare row by row (both should have same number of rows)
            assert_eq!(
                written_transactions.len(),
                original_transactions.len(),
                "Transaction row count should match"
            );

            // Compare each cell value
            for (row_idx, (written_row, original_row)) in written_transactions
                .iter()
                .zip(original_transactions.iter())
                .enumerate()
            {
                for (col_idx, (written_val, original_val)) in
                    written_row.iter().zip(original_row.iter()).enumerate()
                {
                    assert_eq!(
                        written_val, original_val,
                        "Mismatch at row {row_idx}, col {col_idx}: \
                         written '{written_val}' != original '{original_val}'"
                    );
                }
            }
        }
    }

    /// The headers a Transactions tab had before this tool owned a column in it.
    const PRE_MIGRATION_HEADERS: &[&str] = &[
        "Date",
        "Description",
        "Amount",
        "Account",
        "Account #",
        "Institution",
        "Account ID",
        "Transaction ID",
        "Category",
    ];

    /// Builds a Transactions tab as the previous release saw it: no sync ID column at all.
    ///
    /// The three `tid-` rows are the ones the datastore was last synced with. The three above them
    /// are new, in the position Tiller puts new rows: at the top. One has no `Transaction ID` at
    /// all, which is what feeds like Apple Card produce, and two carry the same `split:[1]` marker.
    /// Neither can be told apart by that column, which is the whole reason for the sync ID.
    fn pre_migration_sheet() -> TestSheetState {
        let transactions = vec![
            PRE_MIGRATION_HEADERS.to_vec(),
            vec![
                "2025-03-01",
                "New Blank ID",
                "-11.00",
                "Checking",
                "1234",
                "Test Bank",
                "acct-001",
                "",
                "Food",
            ],
            vec![
                "2025-03-02",
                "New Split A",
                "-12.00",
                "Checking",
                "1234",
                "Test Bank",
                "acct-001",
                "split:[1]",
                "Food",
            ],
            vec![
                "2025-03-03",
                "New Split B",
                "-13.00",
                "Checking",
                "1234",
                "Test Bank",
                "acct-001",
                "split:[1]",
                "Food",
            ],
            vec![
                "2025-01-15",
                "Coffee Shop",
                "-4.50",
                "Checking",
                "1234",
                "Test Bank",
                "acct-001",
                "tid-aaa",
                "Food",
            ],
            vec![
                "2025-01-16",
                "Bookstore",
                "-20.00",
                "Checking",
                "1234",
                "Test Bank",
                "acct-001",
                "tid-bbb",
                "Food",
            ],
            vec![
                "2025-01-17",
                "Groceries",
                "-64.00",
                "Checking",
                "1234",
                "Test Bank",
                "acct-001",
                "tid-ccc",
                "Food",
            ],
        ];

        let categories = vec![
            vec!["Category", "Group", "Type", "Hide From Reports"],
            vec!["Food", "Living", "Expense", ""],
        ];

        let auto_cat = vec![vec!["Category", "Description Contains"]];

        let mut data = std::collections::HashMap::new();
        data.insert(TRANSACTIONS.to_string(), to_grid(transactions));
        data.insert(CATEGORIES.to_string(), to_grid(categories));
        data.insert(AUTO_CAT.to_string(), to_grid(auto_cat));

        TestSheetState {
            data,
            formulas: std::collections::HashMap::new(),
            call_history: std::cell::RefCell::new(vec![]),
        }
    }

    fn to_grid(rows: Vec<Vec<&str>>) -> Vec<Vec<String>> {
        rows.into_iter()
            .map(|row| row.into_iter().map(|s| s.to_string()).collect())
            .collect()
    }

    /// Replaces the datastore with one built the way the previous release built it: schema version
    /// 2, `transactions` keyed on Tiller's `Transaction ID`, and no sync ID anywhere.
    async fn write_pre_migration_datastore(sqlite_path: &std::path::Path) {
        std::fs::remove_file(sqlite_path).unwrap();
        let pool = crate::db::Db::create_at_version(sqlite_path, 2)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO categories (category, category_group, type, hide_from_reports, \
             original_order) VALUES ('Food', 'Living', 'Expense', '', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let rows = [
            ("tid-aaa", "2025-01-15", "Coffee Shop", -4.50, 0),
            ("tid-bbb", "2025-01-16", "Bookstore", -20.00, 1),
            ("tid-ccc", "2025-01-17", "Groceries", -64.00, 2),
        ];
        for (id, date, description, amount, order) in rows {
            sqlx::query(
                "INSERT INTO transactions (transaction_id, date, description, amount, account, \
                 account_number, institution, account_id, category, original_order) \
                 VALUES (?, ?, ?, ?, 'Checking', '1234', 'Test Bank', 'acct-001', 'Food', ?)",
            )
            .bind(id)
            .bind(date)
            .bind(description)
            .bind(amount)
            .bind(order)
            .execute(&pool)
            .await
            .unwrap();
        }

        // A real pre-migration datastore has the header mapping its last `sync down` recorded.
        let metadata: [(&str, &[&str]); 3] = [
            (TRANSACTIONS, PRE_MIGRATION_HEADERS),
            (
                CATEGORIES,
                &["Category", "Group", "Type", "Hide From Reports"],
            ),
            (AUTO_CAT, &["Category", "Description Contains"]),
        ];
        for (sheet, headers) in metadata {
            for (order, header) in headers.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO sheet_metadata (sheet, column_name, header_name, \"order\") \
                     VALUES (?, ?, ?, ?)",
                )
                .bind(sheet)
                .bind(
                    header
                        .to_lowercase()
                        .replace(' ', "_")
                        .replace('#', "number"),
                )
                .bind(*header)
                .bind(order as i64)
                .execute(&pool)
                .await
                .unwrap();
            }
        }

        pool.close().await;
    }

    /// Reads the sync ID column out of the test sheet, keyed by the row's `Transaction ID`.
    fn sheet_sync_ids(env: &TestEnv) -> std::collections::BTreeMap<String, String> {
        let state = env.get_state();
        let grid = state.data.get(TRANSACTIONS).unwrap();
        let header = &grid[0];
        let sync_col = header
            .iter()
            .position(|h| h == crate::model::SYNC_ID_STR)
            .expect("the sheet should have gained the sync ID column");
        let id_col = header.iter().position(|h| h == "Transaction ID").unwrap();

        grid[1..]
            .iter()
            .map(|row| {
                let transaction_id = row.get(id_col).cloned().unwrap_or_default();
                let sync_id = row.get(sync_col).cloned().unwrap_or_default();
                (format!("{transaction_id}|{}", row[1]), sync_id)
            })
            .collect()
    }

    /// A datastore and sheet from before sync IDs existed must come through the upgrade intact,
    /// including the rows Tiller added to the sheet in the meantime.
    ///
    /// This is the case that matters most: a sheet that worked under the previous release, whose
    /// `Transaction ID` values were all unique, has to keep the identity it already had. If the
    /// migration and the sheet bootstrap disagreed about what a row's identifier is, every
    /// existing row would be deleted and re-inserted on the first `sync down`, taking whatever the
    /// user had categorized or annotated with it.
    #[tokio::test]
    async fn test_upgrade_from_a_pre_migration_datastore() {
        let env = TestEnv::new().await;
        let sqlite_path = env.config().sqlite_path().to_path_buf();
        let root = sqlite_path.parent().unwrap().to_path_buf();

        // The sheet as the previous release left it, with three rows Tiller has added since.
        env.set_state(pre_migration_sheet());

        // The datastore as the previous release left it, keyed on `Transaction ID`.
        write_pre_migration_datastore(&sqlite_path).await;

        // Loading the datastore runs the migration.
        let config = Config::load(&root).await.unwrap();
        let migrated: std::collections::BTreeSet<String> = config
            .db()
            .get_tiller_data()
            .await
            .unwrap()
            .transactions
            .data()
            .iter()
            .map(|t| t.sync_id.clone())
            .collect();
        assert_eq!(
            migrated,
            ["sync-tid-aaa", "sync-tid-bbb", "sync-tid-ccc"]
                .iter()
                .map(|s| s.to_string())
                .collect::<std::collections::BTreeSet<String>>(),
            "the migration should derive each sync ID from the transaction ID the row already had"
        );

        // The first sync down after the upgrade: it creates the column and seeds it.
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        let data = config.db().get_tiller_data().await.unwrap();
        let transactions = data.transactions.data();
        assert_eq!(
            transactions.len(),
            6,
            "three rows carried over and three new ones, with nothing lost or duplicated"
        );

        // The rows the datastore already held kept the identifiers the migration gave them, which
        // is what proves they were updated rather than deleted and re-inserted.
        let after: std::collections::BTreeSet<String> =
            transactions.iter().map(|t| t.sync_id.clone()).collect();
        for id in &migrated {
            assert!(
                after.contains(id),
                "{id} should have survived the first sync down, but the datastore now holds {after:?}"
            );
        }

        // The three rows Tiller added were minted, because neither a blank nor a repeated
        // `Transaction ID` can seed anything.
        let minted: Vec<&Transaction> = transactions
            .iter()
            .filter(|t| !migrated.contains(&t.sync_id))
            .collect();
        assert_eq!(minted.len(), 3);
        for transaction in &minted {
            assert!(
                transaction.sync_id.starts_with("sync-"),
                "a minted sync ID should carry the prefix, got '{}'",
                transaction.sync_id
            );
            assert_ne!(transaction.sync_id, "sync-");
            assert_ne!(
                transaction.sync_id, "sync-split:[1]",
                "a repeated Transaction ID must not seed an identifier"
            );
        }

        // Tiller's own values are kept verbatim, repeats and blanks included.
        let mut transaction_ids: Vec<&str> = transactions
            .iter()
            .map(|t| t.transaction_id.as_str())
            .collect();
        transaction_ids.sort();
        assert_eq!(
            transaction_ids,
            vec![
                "",
                "split:[1]",
                "split:[1]",
                "tid-aaa",
                "tid-bbb",
                "tid-ccc"
            ]
        );

        // `original_order` follows the sheet, so the carried-over rows have moved down by three.
        let coffee = transactions
            .iter()
            .find(|t| t.sync_id == "sync-tid-aaa")
            .unwrap();
        assert_eq!(coffee.original_order, Some(3));
        assert_eq!(coffee.description, "Coffee Shop");

        // The sheet was stamped, and every identifier in it matches the datastore.
        let stamped = sheet_sync_ids(&env);
        assert_eq!(stamped.len(), 6);
        assert_eq!(stamped.get("tid-aaa|Coffee Shop").unwrap(), "sync-tid-aaa");
        for (row, sync_id) in &stamped {
            assert!(
                after.contains(sync_id),
                "the sheet holds '{sync_id}' on {row}, which the datastore does not have"
            );
        }

        // And the upgrade settles: a second sync down finds every row already identified, writes
        // nothing, and changes no identifier.
        let sheet = TestSheet::new(config.spreadsheet_id());
        sheet.clear_history();
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        assert!(
            !sheet
                .call_history()
                .iter()
                .any(|call| matches!(call, SheetCall::WriteRanges { .. })),
            "a sync down of an already-identified sheet must not write to it"
        );
        assert_eq!(sheet_sync_ids(&env), stamped);
        let settled: std::collections::BTreeSet<String> = config
            .db()
            .get_tiller_data()
            .await
            .unwrap()
            .transactions
            .data()
            .iter()
            .map(|t| t.sync_id.clone())
            .collect();
        assert_eq!(settled, after);
    }

    /// A column that was already in the user's sheet under our header has to be reported, not
    /// adopted: adopting it would key rows on the user's data and then overwrite that data on the
    /// next `sync up`.
    #[tokio::test]
    async fn test_sync_down_rejects_a_pre_existing_sync_id_column() {
        let env = TestEnv::new().await;
        let config = env.config();

        let mut state = pre_migration_sheet();
        let grid = state.data.get_mut(TRANSACTIONS).unwrap();
        grid[0].push(crate::model::SYNC_ID_STR.to_string());
        for (ix, row) in grid[1..].iter_mut().enumerate() {
            row.push(format!("my own note {ix}"));
        }
        env.set_state(state);

        let err = sync_down(config, Mode::Testing)
            .await
            .expect_err("a column full of the user's data must not be adopted")
            .to_string();

        assert!(err.contains("did not write"), "{err}");
        assert!(err.contains("rename it"), "{err}");
    }

    /// A copy-pasted row leaves two sheet rows sharing one identifier, which `sync down` has to
    /// report rather than guess at.
    #[tokio::test]
    async fn test_sync_down_rejects_duplicate_sync_ids() {
        let env = TestEnv::new().await;
        let config = env.config();

        // A first sync down stamps the sheet.
        sync_down(config.clone(), Mode::Testing).await.unwrap();

        // Copy row 2's identifier onto row 3, as copying a row in the sheet would.
        let mut state = env.get_state();
        let grid = state.data.get_mut(TRANSACTIONS).unwrap();
        let sync_col = grid[0]
            .iter()
            .position(|h| h == crate::model::SYNC_ID_STR)
            .unwrap();
        let copied = grid[1][sync_col].clone();
        grid[2][sync_col] = copied.clone();
        env.set_state(state);

        let err = sync_down(config, Mode::Testing)
            .await
            .expect_err("two rows cannot share one identifier")
            .to_string();

        assert!(err.contains("repeats a value"), "{err}");
        assert!(err.contains(&copied), "{err}");
        assert!(err.contains("rows 2, 3"), "{err}");
    }

    /// The sync ID column travels back to the sheet on `sync up` like any other column.
    #[tokio::test]
    async fn test_sync_up_writes_the_sync_id_column_back() {
        let env = TestEnv::new().await;
        let config = env.config();

        sync_down(config.clone(), Mode::Testing).await.unwrap();
        let stamped = sheet_sync_id_column(&env);
        assert!(!stamped.is_empty());

        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Preserve)
            .await
            .unwrap();

        assert_eq!(
            sheet_sync_id_column(&env),
            stamped,
            "sync up must write every identifier back, unchanged"
        );

        // And the next sync down finds them all and writes nothing.
        let sheet = TestSheet::new(config.spreadsheet_id());
        sheet.clear_history();
        sync_down(config, Mode::Testing).await.unwrap();
        assert!(
            !sheet
                .call_history()
                .iter()
                .any(|call| matches!(call, SheetCall::WriteRanges { .. })),
            "every row is already identified, so nothing should be written"
        );
    }

    /// The sync ID column of the test sheet, in row order.
    fn sheet_sync_id_column(env: &TestEnv) -> Vec<String> {
        let state = env.get_state();
        let grid = state.data.get(TRANSACTIONS).unwrap();
        let col = grid[0]
            .iter()
            .position(|h| h == crate::model::SYNC_ID_STR)
            .expect("the sheet should have the sync ID column");
        grid[1..]
            .iter()
            .map(|row| row.get(col).cloned().unwrap_or_default())
            .collect()
    }
}
