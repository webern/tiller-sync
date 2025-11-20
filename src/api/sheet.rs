//! Implements the `Sheet` trait using the `sheets:Client` to interact with a Google sheet.

use crate::api::{Sheet, TokenProvider};
use crate::{Config, Result};
use anyhow::Context;
use sheets::types::{DateTimeRenderOption, Dimension, ValueRenderOption};

/// Implements the `Sheet` trait using the `sheets:Client` to interact with a Google sheet. It takes
/// a `TokenProvider`, on which it calls refresh to keep the token up-to-date.
pub(super) struct GoogleSheet {
    config: Config,
    token_provider: TokenProvider,
    client: sheets::Client,
}

impl GoogleSheet {
    pub(super) async fn new(config: Config, mut token_provider: TokenProvider) -> Result<Self> {
        let client = create_sheets_client(&mut token_provider).await?;
        Ok(Self {
            config,
            token_provider,
            client,
        })
    }

    /// Refreshes the sheets client with a new access token if needed
    async fn refresh_client(&mut self) -> Result<()> {
        self.client = create_sheets_client(&mut self.token_provider).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Sheet for GoogleSheet {
    async fn get(&mut self, sheet_name: &str) -> Result<Vec<Vec<String>>> {
        self.refresh_client().await?;
        let range = format!("{sheet_name}!A:ZZ"); // Get all columns
        let response = self
            .client
            .spreadsheets()
            .values_get(
                self.config.spreadsheet_id(),
                &range,
                DateTimeRenderOption::FormattedString,
                Dimension::Rows,
                ValueRenderOption::FormattedValue,
            )
            .await
            .with_context(|| format!("Failed to fetch {sheet_name} sheet data"))?;
        Ok(response.body.values)
    }

    async fn _put(&mut self, _sheet_name: &str, _data: &[Vec<String>]) -> Result<()> {
        self.refresh_client().await?;
        todo!()
    }
}

/// Creates a new sheets client with a refreshed access token.
async fn create_sheets_client(token_provider: &mut TokenProvider) -> Result<sheets::Client> {
    // Get the access token (will refresh if needed)
    let access_token = token_provider.token_with_refresh().await?;

    // Create sheets client
    // Note: The sheets crate requires client_id, client_secret, and redirect_uri,
    // but we don't need them for API calls, only the access token
    Ok(sheets::Client::new(
        String::new(), // client_id (not needed for API calls with access token)
        String::new(), // client_secret (not needed for API calls with access token)
        String::new(), // redirect_uri (not needed for API calls with access token)
        access_token.to_string(),
        String::new(), // refresh_token (not needed, we handle refresh ourselves)
    ))
}
