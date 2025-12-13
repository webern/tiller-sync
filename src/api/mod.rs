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
use crate::api::sheet_test_client::TestSheet;
use crate::api::tiller::TillerImpl;
use crate::model::{Amount, RowCol, TillerData};
use crate::{Config, Result};
pub(super) use oauth::TokenProvider;
use std::env::VarError;

// OAuth scopes required for Sheets API access and Drive file operations (backup copies)
const OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/drive.file",
];

// These are the sheet tab names that we care about.
pub(crate) const TRANSACTIONS: &str = "Transactions";
pub(crate) const CATEGORIES: &str = "Categories";
pub(crate) const AUTO_CAT: &str = "AutoCat";

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
    pub fn from_env() -> Self {
        match std::env::var(MODE_ENV) {
            Err(VarError::NotPresent) => Self::Google,
            _ => Self::Testing,
        }
    }
}

/// Construct a `Sheet` object and select the `Mode`: either testing or live. Your can pass
/// `Mode::from_env()` for the mode parameter to let the application choose live or testing mode
/// based on the presence or absence of the `TILLER_SYNC_IN_TEST_MODE` environment variable.
pub async fn sheet(conf: Config, token_provider: TokenProvider, m: Mode) -> Result<Box<dyn Sheet>> {
    match m {
        Mode::Google => Ok(Box::new(GoogleSheet::new(conf, token_provider).await?)),
        Mode::Testing => Ok(Box::new(TestSheet::default())),
    }
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

    /// Replace the data in a Google sheet.
    // TODO: this function signature might need to change depending on how we plan to merge data.
    async fn _put(&mut self, sheet_name: &str, data: &[Vec<String>]) -> Result<()>;
}

#[async_trait::async_trait]
pub trait Tiller {
    /// Get the data from the Tiller Google sheet.
    async fn get_data(&mut self) -> Result<TillerData>;
}

#[tokio::test]
async fn test_sync_down_behavior() {
    use std::str::FromStr;

    let client = Box::new(TestSheet::default());
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
