//! Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.

use crate::api::{Sheet, SheetRange, Tiller, AUTO_CAT, CATEGORIES, TRANSACTIONS};
use crate::model::{AutoCats, Categories, TillerData, Transactions};
use crate::Result;

/// Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.
pub(super) struct TillerImpl {
    sheet: Box<dyn Sheet + Send>,
}

impl TillerImpl {
    /// Create a new `TillerImpl` object that will use a dynamically-dispatched `sheet` to get and
    /// send its data.
    pub(super) async fn new(sheet: Box<dyn Sheet + Send>) -> Result<Self> {
        Ok(Self { sheet })
    }
}

#[async_trait::async_trait]
impl Tiller for TillerImpl {
    async fn get_data(&mut self) -> Result<TillerData> {
        // Fetch data from all three tabs
        let transactions = fetch_transactions(self.sheet.as_mut()).await?;
        let categories = fetch_categories(self.sheet.as_mut()).await?;
        let auto_cats = fetch_auto_cats(self.sheet.as_mut()).await?;

        Ok(TillerData {
            transactions,
            categories,
            auto_cats,
        })
    }

    async fn copy_spreadsheet(&mut self, new_name: &str) -> Result<String> {
        self.sheet.copy_spreadsheet(new_name).await
    }

    async fn clear_and_write_data(&mut self, data: &TillerData) -> Result<()> {
        // Clear each tab entirely (headers and data)
        let clear_ranges = [
            &format!("{TRANSACTIONS}!A1:ZZ"),
            &format!("{CATEGORIES}!A1:ZZ"),
            &format!("{AUTO_CAT}!A1:ZZ"),
        ];
        self.sheet
            .clear_ranges(&clear_ranges.map(|s| s.as_str()))
            .await?;

        // Build write data for all three sheets (headers + data in one operation each)
        let mut write_data = Vec::new();

        // Transactions - all rows (header + data)
        let txn_data = data.transactions.to_rows()?;
        write_data.push(SheetRange {
            range: format!("{TRANSACTIONS}!A1:ZZ"),
            values: txn_data,
        });

        // Categories - all rows (header + data)
        let cat_data = data.categories.to_rows()?;
        write_data.push(SheetRange {
            range: format!("{CATEGORIES}!A1:ZZ"),
            values: cat_data,
        });

        // AutoCat - all rows (header + data)
        let aut_data = data.auto_cats.to_rows()?;
        write_data.push(SheetRange {
            range: format!("{AUTO_CAT}!A1:ZZ"),
            values: aut_data,
        });

        self.sheet.write_ranges(&write_data).await?;

        Ok(())
    }

    async fn verify_write(&mut self, expected: &TillerData) -> Result<(usize, usize, usize)> {
        use anyhow::bail;

        // Re-fetch data from sheets to verify row counts
        let actual = self.get_data().await?;

        let expected_txn = expected.transactions.data().len();
        let expected_cat = expected.categories.data().len();
        let expected_ac = expected.auto_cats.data().len();

        let actual_txn = actual.transactions.data().len();
        let actual_cat = actual.categories.data().len();
        let actual_ac = actual.auto_cats.data().len();

        if actual_txn != expected_txn {
            bail!(
                "Verification failed: expected {} transactions, found {}",
                expected_txn,
                actual_txn
            );
        }

        if actual_cat != expected_cat {
            bail!(
                "Verification failed: expected {} categories, found {}",
                expected_cat,
                actual_cat
            );
        }

        if actual_ac != expected_ac {
            bail!(
                "Verification failed: expected {} autocat rules, found {}",
                expected_ac,
                actual_ac
            );
        }

        Ok((actual_txn, actual_cat, actual_ac))
    }
}

/// Fetches transaction data from the Transactions tab
async fn fetch_transactions(client: &mut (dyn Sheet + Send)) -> Result<Transactions> {
    let values = client.get(TRANSACTIONS).await?;
    let formulas = client.get_formulas(TRANSACTIONS).await?;
    Transactions::parse(values, formulas)
}

/// Fetches category data from the Categories tab
async fn fetch_categories(client: &mut (dyn Sheet + Send)) -> Result<Categories> {
    let values = client.get(CATEGORIES).await?;
    let formulas = client.get_formulas(CATEGORIES).await?;
    Categories::parse(values, formulas)
}

/// Fetches AutoCat data from the AutoCat tab
async fn fetch_auto_cats(client: &mut (dyn Sheet + Send)) -> Result<AutoCats> {
    let values = client.get(AUTO_CAT).await?;
    let formulas = client.get_formulas(AUTO_CAT).await?;
    AutoCats::parse(values, formulas)
}
