use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CsvTransactionType {
    Credit,
    Debit,
}

impl Default for CsvTransactionType {
    fn default() -> Self {
        CsvTransactionType::Credit
    }
}

// "Date","Description","Original Description","Amount","Transaction Type","Category","Account Name","Labels","Notes"
#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct CsvRecord {
    pub(crate) date: String,
    pub(crate) description: String,
    #[serde(rename = "Original Description")]
    pub(crate) original_description: String,
    pub(crate) amount: String,
    #[serde(rename = "Transaction Type")]
    pub(crate) transaction_type: CsvTransactionType,
    pub(crate) category: String,
    #[serde(rename = "Account Name")]
    pub(crate) account_name: String,
    pub(crate) labels: String,
    pub(crate) notes: String,
}
