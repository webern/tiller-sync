use crate::model::Amount;
use serde::{Deserialize, Serialize};

/// Represents a single row from the AutoCat sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AutoCat {
    pub(crate) category: String,
    pub(crate) description_contains: Option<String>,
    pub(crate) account_contains: Option<String>,
    pub(crate) institution_contains: Option<String>,
    pub(crate) amount_min: Option<Amount>,
    pub(crate) amount_max: Option<Amount>,
    pub(crate) amount_equals: Option<Amount>,
    pub(crate) description_equals: Option<String>,
    pub(crate) description_full: Option<String>,
    pub(crate) full_description_contains: Option<String>,
    pub(crate) amount_contains: Option<String>,
}
