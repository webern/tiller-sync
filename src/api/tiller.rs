//! Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.

use crate::api::{Sheet, SheetRange, Tiller, WriteCounts, AUTO_CAT, CATEGORIES, TRANSACTIONS};
use crate::error::Res;
use crate::model::{resolve_transaction_ids, AutoCats, Categories, TillerData, Transactions};
use tracing::debug;

/// Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.
pub(super) struct TillerImpl {
    sheet: Box<dyn Sheet + Send>,
}

impl TillerImpl {
    /// Create a new `TillerImpl` object that will use a dynamically-dispatched `sheet` to get and
    /// send its data.
    pub(super) async fn new(sheet: Box<dyn Sheet + Send>) -> Res<Self> {
        Ok(Self { sheet })
    }
}

#[async_trait::async_trait]
impl Tiller for TillerImpl {
    async fn get_data(&mut self) -> Res<TillerData> {
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

    async fn copy_spreadsheet(&mut self, new_name: &str) -> Res<String> {
        self.sheet.copy_spreadsheet(new_name).await
    }

    async fn clear_and_write_data(
        &mut self,
        data: &TillerData,
        preserve_formulas: bool,
    ) -> Res<usize> {
        // Clear each tab entirely (headers and data)
        let clear_ranges = [
            &format!("{TRANSACTIONS}!A1:ZZ"),
            &format!("{CATEGORIES}!A1:ZZ"),
            &format!("{AUTO_CAT}!A1:ZZ"),
        ];
        self.sheet
            .clear_ranges(&clear_ranges.map(|s| s.as_str()))
            .await?;

        // Build write data for all three sheets (headers + data in one operation each). Formulas
        // are overlaid onto the same grid rather than written separately, so a single write carries
        // both the values and the formulas and there is no window in which the sheet holds one but
        // not the other.
        let mut write_data = Vec::new();
        let mut formulas_written = 0;

        let tabs = [
            (
                TRANSACTIONS,
                data.transactions.to_rows_for_write(preserve_formulas),
            ),
            (
                CATEGORIES,
                data.categories.to_rows_for_write(preserve_formulas),
            ),
            (
                AUTO_CAT,
                data.auto_cats.to_rows_for_write(preserve_formulas),
            ),
        ];

        for (tab, rows) in tabs {
            let (values, count) = rows?;
            if preserve_formulas {
                debug!("Writing {count} formulas to the {tab} tab");
            }
            formulas_written += count;
            write_data.push(SheetRange {
                range: format!("{tab}!A1:ZZ"),
                values,
            });
        }

        self.sheet.write_ranges(&write_data).await?;

        Ok(formulas_written)
    }

    async fn verify_write(&mut self, expected: &TillerData) -> Res<WriteCounts> {
        use anyhow::bail;

        // Re-fetch data from sheets to verify row counts
        let actual = self.get_data().await?;

        let expected_txn = expected.transactions.data().len();
        let expected_cat = expected.categories.data().len();
        let expected_ac = expected.auto_cats.data().len();

        let counts = WriteCounts {
            transactions: actual.transactions.data().len(),
            categories: actual.categories.data().len(),
            auto_cats: actual.auto_cats.data().len(),
            formulas: actual.transactions.formulas().len()
                + actual.categories.formulas().len()
                + actual.auto_cats.formulas().len(),
        };

        if counts.transactions != expected_txn {
            bail!(
                "Verification failed: expected {} transactions, found {}",
                expected_txn,
                counts.transactions
            );
        }

        if counts.categories != expected_cat {
            bail!(
                "Verification failed: expected {} categories, found {}",
                expected_cat,
                counts.categories
            );
        }

        if counts.auto_cats != expected_ac {
            bail!(
                "Verification failed: expected {} autocat rules, found {}",
                expected_ac,
                counts.auto_cats
            );
        }

        Ok(counts)
    }
}

/// Fetches transaction data from the Transactions tab
async fn fetch_transactions(client: &mut (dyn Sheet + Send)) -> Res<Transactions> {
    let values = client.get(TRANSACTIONS).await?;
    let formulas = client.get_formulas(TRANSACTIONS).await?;
    let mut transactions = Transactions::parse(values, formulas)?;

    // Rows whose Transaction ID is blank or duplicated cannot be keyed on that value. Resolving
    // here rather than in the sync commands means every read of the sheet is normalized the same
    // way, so the conflict-detection snapshot and the verification read-back stay comparable.
    resolve_transaction_ids(&mut transactions).log();

    Ok(transactions)
}

/// Fetches category data from the Categories tab
async fn fetch_categories(client: &mut (dyn Sheet + Send)) -> Res<Categories> {
    let values = client.get(CATEGORIES).await?;
    let formulas = client.get_formulas(CATEGORIES).await?;
    Categories::parse(values, formulas)
}

/// Fetches AutoCat data from the AutoCat tab
async fn fetch_auto_cats(client: &mut (dyn Sheet + Send)) -> Res<AutoCats> {
    let values = client.get(AUTO_CAT).await?;
    let formulas = client.get_formulas(AUTO_CAT).await?;
    AutoCats::parse(values, formulas)
}
