use crate::api::{Mode, Tiller, TokenProvider};
use crate::backup::{SYNC_DOWN, SYNC_UP_PRE};
use crate::model::TillerData;
use crate::{Config, Result};
use anyhow::bail;
use log::debug;

pub async fn sync_down(config: Config) -> Result<()> {
    let token_provider = token_provider(&config).await?;
    let tiller_data = get_data(&config, token_provider).await?;

    // Save backup immediately after download
    let backup_path = config.backup().save_json(SYNC_DOWN, &tiller_data).await?;
    debug!("Saved backup to {}", backup_path.display());

    // TODO: Write to SQLite database

    // TODO: Remove this print to stdout when we have implemented writing to SQLite
    let s = serde_json::to_string_pretty(&tiller_data)?;
    println!("{s}");

    Ok(())
}

pub async fn sync_up(config: Config) -> Result<()> {
    // Precondition: verify database has transactions
    if config.db().count_transactions()? == 0 {
        bail!("Database has no transactions, run 'tiller sync down' to get data");
    }

    // TODO: Download current sheet state
    let token_provider = token_provider(&config).await?;
    let current_sheet = get_data(&config, token_provider).await?;

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

async fn get_data(config: &Config, token_provider: TokenProvider) -> Result<TillerData> {
    let client = crate::api::sheet(config.clone(), token_provider, Mode::from_env()).await?;
    let mut tiller = crate::api::tiller(client).await?;
    tiller.get_data().await
}

async fn token_provider(config: &Config) -> Result<TokenProvider> {
    TokenProvider::load(config.client_secret_path(), config.token_path()).await
}
