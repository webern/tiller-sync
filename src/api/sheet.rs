//! Implements the `Sheet` trait using the `sheets:Client` to interact with a Google sheet.

use crate::api::{Sheet, SheetRange, TokenProvider};
use crate::error::Res;
use crate::Config;
use anyhow::Context;
use sheets::types::{
    BatchClearValuesRequest, BatchUpdateValuesRequest, DateTimeRenderOption, Dimension,
    ValueInputOption, ValueRange, ValueRenderOption,
};
use sheets::ClientError;
use tracing::trace;

/// Implements the `Sheet` trait using the `sheets:Client` to interact with a Google sheet. It takes
/// a `TokenProvider`, on which it calls refresh to keep the token up-to-date.
pub(super) struct GoogleSheet {
    config: Config,
    token_provider: TokenProvider,
    client: sheets::Client,
}

impl GoogleSheet {
    pub(super) async fn new(config: Config, mut token_provider: TokenProvider) -> Res<Self> {
        let client = create_sheets_client(&mut token_provider).await?;
        Ok(Self {
            config,
            token_provider,
            client,
        })
    }

    /// Refreshes the sheets client with a new access token if needed
    async fn refresh_client(&mut self) -> Res<()> {
        self.client = create_sheets_client(&mut self.token_provider).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Sheet for GoogleSheet {
    async fn get(&mut self, sheet_name: &str) -> Res<Vec<Vec<String>>> {
        trace!("get for {sheet_name}");
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
            .map_err(map_client_error)
            .with_context(|| format!("Failed to fetch {sheet_name} sheet data"))?;
        Ok(response.body.values)
    }

    async fn get_formulas(&mut self, sheet_name: &str) -> Res<Vec<Vec<String>>> {
        trace!("get_formulas for {sheet_name}");
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
                ValueRenderOption::Formula,
            )
            .await
            .map_err(map_client_error)
            .with_context(|| format!("Failed to fetch {sheet_name} sheet formulas"))?;
        Ok(response.body.values)
    }

    async fn clear_ranges(&mut self, ranges: &[&str]) -> Res<()> {
        self.refresh_client().await?;
        let request = BatchClearValuesRequest {
            ranges: ranges.iter().map(|s| s.to_string()).collect(),
        };
        self.client
            .spreadsheets()
            .values_batch_clear(self.config.spreadsheet_id(), &request)
            .await
            .map_err(map_client_error)
            .with_context(|| format!("Failed to clear ranges: {:?}", ranges))?;
        Ok(())
    }

    async fn write_ranges(&mut self, data: &[SheetRange]) -> Res<()> {
        self.refresh_client().await?;
        let value_ranges: Vec<ValueRange> = data
            .iter()
            .map(|sr| ValueRange {
                major_dimension: Some(Dimension::Rows),
                range: sr.range.clone(),
                values: sr.values.clone(),
            })
            .collect();

        let request = BatchUpdateValuesRequest {
            data: value_ranges,
            include_values_in_response: Some(false),
            response_date_time_render_option: None,
            response_value_render_option: None,
            value_input_option: Some(ValueInputOption::UserEntered),
        };

        self.client
            .spreadsheets()
            .values_batch_update(self.config.spreadsheet_id(), &request)
            .await
            .map_err(map_client_error)
            .with_context(|| "Failed to write ranges")?;
        Ok(())
    }

    async fn copy_spreadsheet(&mut self, new_name: &str) -> Res<String> {
        self.refresh_client().await?;

        // Use Google Drive API to copy the spreadsheet
        // POST https://www.googleapis.com/drive/v3/files/{fileId}/copy
        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}/copy",
            self.config.spreadsheet_id()
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .bearer_auth(self.token_provider.token())
            .json(&serde_json::json!({
                "name": new_name
            }))
            .send()
            .await
            .context("Failed to send copy request to Google Drive API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());
            anyhow::bail!(
                "Google Drive API copy failed with status {}: {}",
                status,
                body
            );
        }

        // Parse response to get the new file ID
        let response_json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Google Drive API response")?;

        let file_id = response_json
            .get("id")
            .and_then(|v| v.as_str())
            .context("Google Drive API response missing 'id' field")?
            .to_string();

        Ok(file_id)
    }
}

/// Creates a new sheets client with a refreshed access token.
async fn create_sheets_client(token_provider: &mut TokenProvider) -> Res<sheets::Client> {
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

fn map_client_error(e: sheets::ClientError) -> anyhow::Error {
    let error_name = match &e {
        ClientError::EmptyRefreshToken => "EmptyRefreshToken".to_string(),
        ClientError::FromUtf8Error(inner) => format!("FromUtf8Error {inner}"),
        ClientError::UrlParserError(inner) => format!("UrlParserError {inner}"),
        ClientError::SerdeJsonError(inner) => format!("SerdeJsonError {inner}"),
        ClientError::ReqwestError(inner) => format!("ReqwestError {inner}"),
        ClientError::InvalidHeaderValue(inner) => format!("InvalidHeaderValue {inner}"),
        ClientError::ReqwestMiddleWareError(inner) => format!("ReqwestMiddleWareError {inner}"),
        ClientError::HttpError { .. } => "HttpError".to_string(),
        ClientError::Other(_) => "Other".to_string(),
    };
    Err::<(), ClientError>(e).context(error_name).err().unwrap()
}
