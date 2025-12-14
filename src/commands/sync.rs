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

pub async fn sync_up(config: Config) -> Result<()> {
    // Validate that we have a valid database with existing transactions.
    if config.db().count_transactions()? == 0 {
        bail!("Database has no transactions, run 'tiller sync down' to get data");
    }

    todo!();
}
