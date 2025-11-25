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

    async fn _put(&mut self, sheet_name: &str, data: &[Vec<String>]) -> Result<()> {
        self.refresh_client().await?;

        // Use the sheets API to update values
        // The range A1:ZZ covers all columns we need
        let range = format!("{sheet_name}!A1:ZZ");

        // Convert our data to Vec<Vec<String>>
        let values: Vec<Vec<String>> = data.to_vec();

        // Create the value range
        let value_range = sheets::types::ValueRange {
            major_dimension: Some(sheets::types::Dimension::Rows),
            range: range.clone(),
            values,
        };

        // Update the values in the sheet
        // values_update requires: spreadsheet_id, range, include_values_in_response,
        // response_date_time_render_option, response_value_render_option, value_input_option, body
        self.client
            .spreadsheets()
            .values_update(
                self.config.spreadsheet_id(),
                &range,
                false, // include_values_in_response
                DateTimeRenderOption::FormattedString,
                ValueRenderOption::FormattedValue,
                sheets::types::ValueInputOption::Raw,
                &value_range,
            )
            .await
            .with_context(|| format!("Failed to update {sheet_name} sheet data"))?;

        Ok(())
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
