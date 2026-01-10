//! Types that represent the core data model, such as `Transaction` and `Category`.
mod amount;
mod auto_cat;
mod category;
mod items;
mod mapping;
mod row_col;
mod transaction;

pub use amount::{Amount, AmountFormat};
pub use auto_cat::{AutoCat, AutoCatUpdates, AutoCats};
pub use category::{Categories, Category, CategoryUpdates};
pub(crate) use items::Item;
pub(crate) use mapping::Mapping;
pub(crate) use row_col::RowCol;
use serde::{Deserialize, Serialize};
pub use transaction::{Transaction, TransactionColumn, TransactionUpdates, Transactions};

/// Represents all the sheets of interest from a tiller Google sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TillerData {
    // TODO: make these private again
    /// Rows of data from the Transactions sheet.
    pub(crate) transactions: Transactions,
    /// Rows of data from the Categories sheet.
    pub(crate) categories: Categories,
    /// Rows of data from the AutoCat sheet.
    pub(crate) auto_cats: AutoCats,
}

impl TillerData {
    /// Returns true if any of the sheets contain formulas.
    pub(crate) fn has_formulas(&self) -> bool {
        !self.transactions.formulas().is_empty()
            || !self.categories.formulas().is_empty()
            || !self.auto_cats.formulas().is_empty()
    }

    /// Checks if any of the sheets have gaps in their `original_order` sequences.
    ///
    /// Gaps indicate deleted rows (e.g., sequence 0, 1, 3 is missing 2).
    /// This is important for formula preservation since formulas are position-dependent.
    pub(crate) fn has_original_order_gaps(&self) -> bool {
        // Check transactions
        if Self::check_gaps(self.transactions.data().iter().map(|t| t.original_order)) {
            return true;
        }
        // Check categories
        if Self::check_gaps(self.categories.data().iter().map(|c| c.original_order)) {
            return true;
        }
        // Check autocat
        if Self::check_gaps(self.auto_cats.data().iter().map(|c| c.original_order)) {
            return true;
        }
        false
    }

    /// Helper to check for gaps in a sequence of original_order values.
    fn check_gaps(orders: impl Iterator<Item = Option<u64>>) -> bool {
        let mut orders: Vec<u64> = orders.flatten().collect();

        if orders.is_empty() {
            return false;
        }

        orders.sort();

        for (i, &order) in orders.iter().enumerate() {
            if order != i as u64 {
                return true;
            }
        }

        false
    }
}
