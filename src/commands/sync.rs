use crate::api::{Mode, Tiller, TokenProvider};
use crate::{Config, Result};
use anyhow::bail;

pub async fn sync_down(config: Config) -> Result<()> {
    let token_provider =
        TokenProvider::load(config.client_secret_path(), config.token_path()).await?;
    let client = crate::api::sheet(config, token_provider, Mode::from_env()).await?;
    let mut tiller = crate::api::tiller(client).await?;
    let tiller_sheet = tiller.get_data().await?;
    let s = serde_json::to_string_pretty(&tiller_sheet).unwrap();
    println!("{s}");
    Ok(())
}

/// Sync up: Upload data from local database to Google Sheets
///
/// NOTE: This is a basic implementation that demonstrates the sync up flow.
/// A complete implementation would need:
/// - SQLite database layer to read local data
/// - Backup creation before upload
/// - Conflict detection (compare remote vs last sync state)
/// - Force flag handling
/// - Proper error recovery
pub async fn sync_up(_config: Config) -> Result<()> {
    // TODO: This is a placeholder implementation
    // A real implementation needs:
    // 1. Database layer to read from SQLite
    // 2. Backup creation
    // 3. Conflict detection
    // 4. Transaction safety

    bail!(
        "sync up is not yet fully implemented. \
         The basic Sheet::_put() method is ready, but the database layer \
         and sync up logic still need to be implemented. \
         See docs/DESIGN.md for the planned implementation."
    );

    // The code below shows what the implementation will look like once the database layer exists:
    //
    // let token_provider =
    //     TokenProvider::load(config.client_secret_path(), config.token_path()).await?;
    // let mut sheet = crate::api::sheet(config.clone(), token_provider, Mode::from_env()).await?;
    //
    // // 1. Validate local database is not empty
    // let transaction_count = database.count_transactions()?;
    // if transaction_count == 0 {
    //     bail!("Local database is empty. Run 'tiller sync down' first.");
    // }
    //
    // // 2. Create backup
    // database.create_backup()?;
    //
    // // 3. Fetch current remote state for conflict detection
    // let remote_transactions = sheet.get(TRANSACTIONS).await?;
    // let remote_categories = sheet.get(CATEGORIES).await?;
    // let remote_autocats = sheet.get(AUTO_CAT).await?;
    //
    // // 4. Detect conflicts (if remote differs from last_sync_state)
    // if has_conflicts(&remote_transactions, &last_sync_state.transactions) {
    //     if !force_flag {
    //         bail!("External changes detected. Use --force to overwrite, or sync down first.");
    //     }
    // }
    //
    // // 5. Convert database rows to sheet format
    // let local_transactions = database.get_all_transactions_as_rows()?;
    // let local_categories = database.get_all_categories_as_rows()?;
    // let local_autocats = database.get_all_autocats_as_rows()?;
    //
    // // 6. Upload to sheets
    // sheet._put(TRANSACTIONS, &local_transactions).await?;
    // sheet._put(CATEGORIES, &local_categories).await?;
    // sheet._put(AUTO_CAT, &local_autocats).await?;
    //
    // println!("✓ Successfully synced to Google Sheets:");
    // println!("  - {} transactions", local_transactions.len() - 1);
    // println!("  - {} categories", local_categories.len() - 1);
    // println!("  - {} AutoCat rules", local_autocats.len() - 1);
    //
    // Ok(())
}
