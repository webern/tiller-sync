use serde::{Deserialize, Serialize};

/// Represents a single row from the Category sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Category {
    // TODO: make these private again
    pub(crate) category: String,
    pub(crate) group: String,
    #[serde(rename = "type")]
    pub(crate) _type: String,
    pub(crate) hide_from_reports: String,
}
