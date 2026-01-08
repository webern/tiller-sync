//! Shared test utilities for creating test environments.
//!
//! This module is only compiled when running tests (`#[cfg(test)]`).

use crate::api::{TestSheet, TestSheetState};
use crate::model::TillerData;
use crate::model::{AutoCats, Categories, Transactions};
use crate::Config;
use tempfile::TempDir;
use uuid::Uuid;

/// Test environment that sets up a tiller home directory with Config and database.
/// Holds TempDir to keep the directory alive for the duration of the test.
pub struct TestEnv {
    _temp_dir: TempDir,
    config: Config,
}

impl TestEnv {
    /// Creates a test environment with Config and initialized database.
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().join("tiller");
        let secret_path = temp_dir.path().join("client_secret.json");

        // Create minimal client_secret.json
        let secret_content = r#"{
            "installed": {
                "client_id": "test-client-id",
                "client_secret": "test-secret",
                "redirect_uris": ["http://localhost"],
                "auth_uri": "https://accounts.google.com/o/oauth2/auth",
                "token_uri": "https://oauth2.googleapis.com/token"
            }
        }"#;
        std::fs::write(&secret_path, secret_content).unwrap();

        let rand = Uuid::new_v4().to_string().replace('-', "");
        let sheet_url = format!("https://docs.google.com/spreadsheets/d/{}/edit", rand);
        let config = Config::create(&root, &secret_path, &sheet_url)
            .await
            .unwrap();

        Self {
            _temp_dir: temp_dir,
            config,
        }
    }

    /// Returns a clone of the Config.
    pub fn config(&self) -> Config {
        self.config.clone()
    }

    /// Gets the current state of the TestSheet associated with this environment.
    pub fn get_state(&self) -> TestSheetState {
        let test_sheet = TestSheet::new(self.config.spreadsheet_id());
        test_sheet.get_state()
    }

    /// Sets the state of the TestSheet associated with this environment.
    pub fn set_state(&self, state: TestSheetState) {
        let test_sheet = TestSheet::new(self.config.spreadsheet_id());
        test_sheet.set_state(state)
    }

    /// Inserts test transaction data into the database.
    ///
    /// Creates a transaction with the given ID along with the categories needed
    /// to satisfy foreign key constraints.
    pub async fn insert_test_transaction(&self, transaction_id: &str) {
        let transactions = Transactions::parse(
            vec![
                vec![
                    "Transaction ID",
                    "Date",
                    "Description",
                    "Amount",
                    "Account",
                    "Account #",
                    "Institution",
                    "Account ID",
                    "Category",
                    "Note",
                ],
                vec![
                    transaction_id,
                    "2025-01-15",
                    "Coffee Shop",
                    "-4.50",
                    "Checking",
                    "1234",
                    "Test Bank",
                    "acct-001",
                    "Food",
                    "morning coffee",
                ],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let categories = Categories::parse(
            vec![
                vec!["Category", "Group", "Type", "Hide From Reports"],
                vec!["Food", "Living", "Expense", ""],
                vec!["Entertainment", "Fun", "Expense", ""],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let auto_cats = AutoCats::parse(
            vec![vec!["Category", "Description Contains"]],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let data = TillerData {
            transactions,
            categories,
            auto_cats,
        };

        self.config.db().save_tiller_data(&data).await.unwrap();
    }

    /// Inserts test AutoCat data into the database.
    ///
    /// Creates AutoCat rules along with the categories needed to satisfy foreign key constraints.
    /// The AutoCat rules get synthetic IDs (1, 2, etc.) assigned by the database.
    pub async fn insert_test_autocat_data(&self) {
        let transactions = Transactions::parse(
            vec![vec![
                "Transaction ID",
                "Date",
                "Description",
                "Amount",
                "Account",
                "Account #",
                "Institution",
                "Account ID",
                "Category",
                "Note",
            ]],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let categories = Categories::parse(
            vec![
                vec!["Category", "Group", "Type", "Hide From Reports"],
                vec!["Food", "Living", "Expense", ""],
                vec!["Entertainment", "Fun", "Expense", ""],
                vec!["Transportation", "Living", "Expense", ""],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let auto_cats = AutoCats::parse(
            vec![
                vec![
                    "Category",
                    "Description Contains",
                    "Account Contains",
                    "Amount Min",
                ],
                vec!["Food", "starbucks,coffee", "", ""],
                vec!["Entertainment", "netflix", "", "10.00"],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let data = TillerData {
            transactions,
            categories,
            auto_cats,
        };

        self.config.db().save_tiller_data(&data).await.unwrap();
    }

    /// Inserts standalone categories into the database without any transactions or autocat rules.
    ///
    /// This is useful for testing category deletion where no foreign key references exist.
    pub async fn insert_standalone_categories(&self, category_names: &[&str]) {
        let transactions = Transactions::parse(
            vec![vec![
                "Transaction ID",
                "Date",
                "Description",
                "Amount",
                "Account",
                "Account #",
                "Institution",
                "Account ID",
                "Category",
                "Note",
            ]],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let mut category_rows: Vec<Vec<&str>> =
            vec![vec!["Category", "Group", "Type", "Hide From Reports"]];
        for name in category_names {
            category_rows.push(vec![name, "Test Group", "Expense", ""]);
        }

        let categories = Categories::parse(category_rows, Vec::<Vec<&str>>::new()).unwrap();

        let auto_cats = AutoCats::parse(
            vec![vec!["Category", "Description Contains"]],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let data = TillerData {
            transactions,
            categories,
            auto_cats,
        };

        self.config.db().save_tiller_data(&data).await.unwrap();
    }
}
