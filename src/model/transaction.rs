use crate::model::Amount;
use serde::{Deserialize, Serialize};

/// Represents a single row from the Transactions sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Transaction {
    // TODO: make these private again
    pub(crate) transaction_id: String,
    pub(crate) date: String,
    pub(crate) description: String,
    pub(crate) amount: Amount,
    pub(crate) account: String,
    pub(crate) account_number: String,
    pub(crate) institution: String,
    pub(crate) month: String,
    pub(crate) week: String,
    pub(crate) full_description: String,
    pub(crate) account_id: String,
    pub(crate) check_number: String,
    pub(crate) date_added: String,
    pub(crate) merchant_name: String,
    pub(crate) category_hint: String,
    pub(crate) category: String,
    pub(crate) note: String,
    pub(crate) tags: String,
}
