//! Types that represent the core data model, such as `Transaction` and `Category`.
mod amount;
mod auto_cat;
mod category;
mod transaction;

pub use amount::{Amount, AmountFormat};
pub use auto_cat::AutoCat;
pub use category::Category;
use serde::{Deserialize, Serialize};
pub use transaction::Transaction;

/// Represents all the sheets of interest from a tiller Google sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TillerData {
    // TODO: make these private again
    /// Rows of data from the Transactions sheet.
    pub(crate) transactions: Vec<Transaction>,
    /// Rows of data from the Categories sheet.
    pub(crate) categories: Vec<Category>,
    /// Rows of data from the AutoCat sheet.
    pub(crate) auto_cats: Vec<AutoCat>,
}
