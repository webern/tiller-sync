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
use crate::{Amount, Config, Result};
pub(crate) use oauth::TokenProvider;
use serde::{Deserialize, Serialize};
use std::env::VarError;

// OAuth scopes required for Sheets API access
const OAUTH_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/drive.readonly",
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

    /// Replace the data in a Google sheet.
    // TODO: this function signature might need to change depending on how we plan to merge data.
    async fn _put(&mut self, sheet_name: &str, data: &[Vec<String>]) -> Result<()>;
}

#[async_trait::async_trait]
pub trait Tiller {
    /// Get the data from the Tiller Google sheet.
    async fn get_data(&mut self) -> Result<TillerData>;
}

/// Represents all the sheets of interest from a tiller Google sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TillerData {
    /// Rows of data from the Transactions sheet.
    transactions: Vec<Transaction>,
    /// Rows of data from the Categories sheet.
    categories: Vec<Category>,
    /// Rows of data from the AutoCat sheet.
    auto_cats: Vec<AutoCat>,
}

/// Represents a single row from the Transactions sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Transaction {
    transaction_id: String,
    date: String,
    description: String,
    amount: Amount,
    account: String,
    account_number: String,
    institution: String,
    month: String,
    week: String,
    full_description: String,
    account_id: String,
    check_number: String,
    date_added: String,
    merchant_name: String,
    category_hint: String,
    category: String,
    note: String,
    tags: String,
}

/// Represents a single row from the Category sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Category {
    category: String,
    group: String,
    #[serde(rename = "type")]
    _type: String,
    hide_from_reports: String,
}

/// Represents a single row from the AutoCat sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AutoCat {
    category: String,
    description_contains: Option<String>,
    account_contains: Option<String>,
    institution_contains: Option<String>,
    amount_min: Option<Amount>,
    amount_max: Option<Amount>,
    amount_equals: Option<Amount>,
    description_equals: Option<String>,
    description_full: Option<String>,
    full_description_contains: Option<String>,
    amount_contains: Option<String>,
}
