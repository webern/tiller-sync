//! This module encapsulates the interactions with the Google OAuth and the Google Sheets API.
//!
//! - A `Sheet` trait exists to abstract away interactions with Google sheets.
//! - A `Tiller` trait exists to abstract away the specifics of a tiller Google sheet.

mod files;
mod oauth;
mod sheet;
mod sheet_test_client;
mod tiller;

use crate::api::sheet::GoogleSheet;
use crate::api::tiller::TillerImpl;
use crate::model::TillerData;
use crate::{Config, Result};
pub(super) use oauth::TokenProvider;
pub(super) use sheet_test_client::TestSheet;
use std::env::VarError;

#[cfg(test)]
pub(super) use sheet_test_client::{SheetCall, TestSheetState};

// OAuth scopes required for Sheets API access and Drive file operations (backup copies)
// Note: `drive` scope (not `drive.file`) is required because `drive.file` only grants access
// to files created by this app, not pre-existing files like the user's Tiller spreadsheet.
const OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/drive",
];

// These are the sheet tab names that we care about.
pub(crate) const TRANSACTIONS: &str = "Transactions";
pub(crate) const CATEGORIES: &str = "Categories";
pub(crate) const AUTO_CAT: &str = "AutoCat";

/// Represents a range of data to write to a sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetRange {
    /// The A1 notation range, e.g., "Transactions!A1:ZZ1"
    pub range: String,
    /// The data to write (rows of cells)
    pub values: Vec<Vec<String>>,
}

/// For testing purposes, this can be placed into the environment to cause the application to use
/// seeded, testing, in-memory data instead of accessing a live Google sheet.
pub(crate) const MODE_ENV: &str = "TILLER_SYNC_IN_TEST_MODE";

/// An enum representing whether the app is in testing mode or using a live Google sheet.
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Mode {
    /// The app is live, using a real Google sheet.
    #[default]
    Google,
    /// The app is in testing mode, using fake, in-memory data.
    Testing,
}

impl Mode {
    /// Check for the environment variable `TILLER_SYNC_IN_TEST_MODE`. If it exists, returns
    /// `Mode::Testing`, if not, returns `Mode::Google`.
    //
    // The idea here is that the `Mode` should be pushed down from `main.rs`. We should never used
    // from_env() in tests. The only place it should ever be used is in `main.rs`.
    pub fn from_env() -> Self {
        match std::env::var(MODE_ENV) {
            Err(VarError::NotPresent) => Self::Google,
            _ => Self::Testing,
        }
    }
}

/// Construct a `Sheet` object based on `mode`.
/// - For `Mode::Google`: creates a token provider and constructs a Google sheet object
/// - For `Mode::Test`: does not construct a token provider and constructs a Test sheet object
pub async fn sheet(config: Config, mode: Mode) -> Result<Box<dyn Sheet>> {
    let sheet_client: Box<dyn Sheet> = match mode {
        Mode::Google => {
            let token_provider =
                TokenProvider::load(config.client_secret_path(), config.token_path()).await?;
            Box::new(GoogleSheet::new(config.clone(), token_provider).await?)
        }
        Mode::Testing => Box::new(TestSheet::new_with_seed_data(config.spreadsheet_id())),
    };

    Ok(sheet_client)
}

/// Construct a `Tiller` client, which will use `sheet` to communicate with Google sheets (or, in
/// testing mode, will use in-memory seed data).
pub async fn tiller(sheet: Box<dyn Sheet>) -> Result<impl Tiller> {
    TillerImpl::new(sheet).await
}

#[async_trait::async_trait]
pub trait Sheet: Send {
    /// Get the data from a Google sheet.
    async fn get(&mut self, sheet_name: &str) -> Result<Vec<Vec<String>>>;

    /// Get the formulas from a Google sheet (returns formulas for formula cells, values for non-formula cells).
    async fn get_formulas(&mut self, sheet_name: &str) -> Result<Vec<Vec<String>>>;

    /// Clear specified ranges in the spreadsheet.
    /// Each range should be in A1 notation, e.g., "Transactions!A2:ZZ".
    async fn clear_ranges(&mut self, ranges: &[&str]) -> Result<()>;

    /// Write data to specified ranges in the spreadsheet.
    /// Uses ValueInputOption::UserEntered so Sheets can parse dates, numbers, and formulas.
    async fn write_ranges(&mut self, data: &[SheetRange]) -> Result<()>;

    /// Create a copy of the spreadsheet using the Google Drive API.
    /// Returns the file ID of the new copy.
    async fn copy_spreadsheet(&mut self, new_name: &str) -> Result<String>;
}

#[async_trait::async_trait]
pub trait Tiller {
    /// Get the data from the Tiller Google sheet.
    async fn get_data(&mut self) -> Result<TillerData>;

    /// Create a backup copy of the spreadsheet.
    /// Returns the file ID of the new copy.
    async fn copy_spreadsheet(&mut self, new_name: &str) -> Result<String>;

    /// Clear and write data to the Google sheet.
    /// This clears all data rows (preserving headers) and writes new data.
    async fn clear_and_write_data(&mut self, data: &TillerData) -> Result<()>;

    /// Verify that the write was successful by re-fetching row counts.
    /// Returns the counts (transactions, categories, autocat) if verification passes.
    async fn verify_write(&mut self, expected: &TillerData) -> Result<(usize, usize, usize)>;
}

#[tokio::test]
async fn test_sync_down_behavior() {
    use crate::model::{Amount, RowCol};
    use std::str::FromStr as _;

    let client = Box::new(TestSheet::new_with_seed_data("test_sync_down_behavior"));
    let mut tiller = crate::api::tiller(client).await.unwrap();
    let tiller_data = tiller.get_data().await.unwrap();

    // Check that the test data is coming through correctly with an =ABS(E1) formula in
    // Custom Column
    for (tix, transaction) in tiller_data.transactions.data().iter().enumerate() {
        let amount = &transaction.amount;
        let abs = Amount::from_str(transaction.other_fields.get("Custom Column").unwrap()).unwrap();
        assert_eq!(amount.value().abs(), abs.value());
        let formula = format!("=ABS(E{})", tix + 2);
        let cix = tiller_data
            .transactions
            .mapping()
            ._header_index("Custom Column")
            .unwrap();
        let formula_cell = RowCol::new(tix, cix);
        let found_formula = tiller_data
            .transactions
            .formulas()
            .get(&formula_cell)
            .unwrap()
            .to_owned();
        assert_eq!(formula, found_formula);
    }

    // Round-trip the JSON serialization/deserialization
    let tiller_data_serialized = serde_json::to_string_pretty(&tiller_data).unwrap();
    let tiller_data_deserialized: TillerData =
        serde_json::from_str(&tiller_data_serialized).unwrap();
    let tiller_data_serialized_again =
        serde_json::to_string_pretty(&tiller_data_deserialized).unwrap();
    assert_eq!(tiller_data, tiller_data_deserialized);
    assert_eq!(tiller_data_serialized, tiller_data_serialized_again)
}
