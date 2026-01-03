use crate::error::Res;
use crate::model::items::{Item, Items};
use crate::model::Amount;
use crate::utils;
use anyhow::bail;
use clap::Parser;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

/// Represents a collection of transactions from a Transactions sheet, including the header mapping.
/// See tiller documentation for more information about the semantic meanings of transaction
/// columns: https://help.tiller.com/en/articles/432681-transactions-sheet-columns>
pub type Transactions = Items<Transaction>;

/// Represents a single row from the Transactions sheet.
/// See tiller documentation for more information about the semantic meanings of transaction
/// columns: https://help.tiller.com/en/articles/432681-transactions-sheet-columns>
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Transaction {
    /// A unique ID assigned to the transaction by Tiller's systems. Critical for support
    /// troubleshooting and must not be deleted.
    pub(crate) transaction_id: String,

    /// The posted date (when the transaction cleared) or transaction date (when the transaction
    /// occurred). Posted date takes priority except for investment accounts.
    pub(crate) date: String,

    /// Cleaned-up merchant information from your bank.
    pub(crate) description: String,

    /// Transaction value where income and credits are positive; expenses and debits are negative.
    pub(crate) amount: Amount,

    /// The account name as it appears on your bank's website or your custom nickname from Tiller
    /// Console.
    pub(crate) account: String,

    /// Last four digits of the bank account number (e.g., "xxxx1102").
    pub(crate) account_number: String,

    /// Financial institution name (e.g., "Bank of America").
    pub(crate) institution: String,

    /// First day of the transaction's month, useful for pivot tables and reporting.
    pub(crate) month: String,

    /// Sunday date of the transaction's week for weekly breakdowns.
    pub(crate) week: String,

    /// Unmodified merchant details directly from your bank, including codes and numbers.
    pub(crate) full_description: String,

    /// A unique ID assigned to your accounts by Tiller's systems. Important for troubleshooting;
    /// do not delete.
    pub(crate) account_id: String,

    /// Check number when available for checks you write.
    pub(crate) check_number: String,

    /// When the transaction was added to the spreadsheet.
    pub(crate) date_added: String,

    /// Normalized merchant name standardizing variants (e.g., "Amazon" for multiple Amazon
    /// formats). Optional automated column.
    pub(crate) merchant_name: String,

    /// Data provider's category suggestion based on merchant knowledge. Optional automated column;
    /// not included in core templates.
    pub(crate) category_hint: String,

    /// User-assigned category. Non-automated by default to promote spending awareness; AutoCat
    /// available for automation.
    pub(crate) category: String,

    /// Custom notes about specific transactions. Leveraged by Category Rollup reports.
    pub(crate) note: String,

    /// User-defined tags for additional transaction categorization.
    pub(crate) tags: String,

    /// Date when AutoCat automatically categorized or updated a transaction. Google Sheets Add-on
    /// column.
    pub(crate) categorized_date: String,

    /// For reconciling transactions to bank statements. Google Sheets Add-on column.
    pub(crate) statement: String,

    /// Supports workflows including CSV imports. Google Sheets Add-on column.
    pub(crate) metadata: String,

    /// Empty column that may appear at Column A.
    pub(crate) no_name: String,

    /// Custom columns not part of the standard Tiller schema.
    pub(crate) other_fields: BTreeMap<String, String>,

    /// Row position from last sync down (0-indexed); None for locally-added rows.
    /// Used for formula preservation during sync up.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) original_order: Option<u64>,
}

impl Item for Transaction {
    fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Res<()>
    where
        S1: AsRef<str>,
        S2: Into<String>,
    {
        let header = header.as_ref();
        let value = value.into();

        match TransactionColumn::from_header(header) {
            Ok(col) => match col {
                TransactionColumn::TransactionId => self.transaction_id = value,
                TransactionColumn::Date => self.date = value,
                TransactionColumn::Description => self.description = value,
                TransactionColumn::Amount => self.amount = Amount::from_str(&value)?,
                TransactionColumn::Account => self.account = value,
                TransactionColumn::AccountNumber => self.account_number = value,
                TransactionColumn::Institution => self.institution = value,
                TransactionColumn::Month => self.month = value,
                TransactionColumn::Week => self.week = value,
                TransactionColumn::FullDescription => self.full_description = value,
                TransactionColumn::AccountId => self.account_id = value,
                TransactionColumn::CheckNumber => self.check_number = value,
                TransactionColumn::DateAdded => self.date_added = value,
                TransactionColumn::MerchantName => self.merchant_name = value,
                TransactionColumn::CategoryHint => self.category_hint = value,
                TransactionColumn::Category => self.category = value,
                TransactionColumn::Note => self.note = value,
                TransactionColumn::Tags => self.tags = value,
                TransactionColumn::CategorizedDate => self.categorized_date = value,
                TransactionColumn::Statement => self.statement = value,
                TransactionColumn::Metadata => self.metadata = value,
                TransactionColumn::NoName => self.no_name = value,
            },
            Err(_) => {
                let _ = self.other_fields.insert(header.to_string(), value);
            }
        }

        Ok(())
    }

    /// Get a field value by its header name.
    fn get_by_header(&self, header: &str) -> String {
        match TransactionColumn::from_header(header) {
            Ok(col) => match col {
                TransactionColumn::TransactionId => self.transaction_id.clone(),
                TransactionColumn::Date => self.date.clone(),
                TransactionColumn::Description => self.description.clone(),
                TransactionColumn::Amount => self.amount.to_string(),
                TransactionColumn::Account => self.account.clone(),
                TransactionColumn::AccountNumber => self.account_number.clone(),
                TransactionColumn::Institution => self.institution.clone(),
                TransactionColumn::Month => self.month.clone(),
                TransactionColumn::Week => self.week.clone(),
                TransactionColumn::FullDescription => self.full_description.clone(),
                TransactionColumn::AccountId => self.account_id.clone(),
                TransactionColumn::CheckNumber => self.check_number.clone(),
                TransactionColumn::DateAdded => self.date_added.clone(),
                TransactionColumn::MerchantName => self.merchant_name.clone(),
                TransactionColumn::CategoryHint => self.category_hint.clone(),
                TransactionColumn::Category => self.category.clone(),
                TransactionColumn::Note => self.note.clone(),
                TransactionColumn::Tags => self.tags.clone(),
                TransactionColumn::CategorizedDate => self.categorized_date.clone(),
                TransactionColumn::Statement => self.statement.clone(),
                TransactionColumn::Metadata => self.metadata.clone(),
                TransactionColumn::NoName => self.no_name.clone(),
            },
            Err(_) => self.other_fields.get(header).cloned().unwrap_or_default(),
        }
    }

    fn set_original_order(&mut self, original_order: u64) {
        self.original_order = Some(original_order);
    }

    fn get_original_order(&self) -> Option<u64> {
        self.original_order
    }
}

impl Transaction {
    /// Set any of the fields on `self` that are set in `update`.
    pub fn merge_updates(&mut self, update: TransactionUpdates) {
        if let Some(x) = update.date {
            self.date = x;
        }
        if let Some(x) = update.description {
            self.description = x;
        }
        if let Some(x) = update.amount {
            self.amount = x;
        }
        if let Some(x) = update.account {
            self.account = x;
        }
        if let Some(x) = update.account_number {
            self.account_number = x;
        }
        if let Some(x) = update.institution {
            self.institution = x;
        }
        if let Some(x) = update.month {
            self.month = x;
        }
        if let Some(x) = update.week {
            self.week = x;
        }
        if let Some(x) = update.full_description {
            self.full_description = x;
        }
        if let Some(x) = update.account_id {
            self.account_id = x;
        }
        if let Some(x) = update.check_number {
            self.check_number = x;
        }
        if let Some(x) = update.date_added {
            self.date_added = x;
        }
        if let Some(x) = update.merchant_name {
            self.merchant_name = x;
        }
        if let Some(x) = update.category_hint {
            self.category_hint = x;
        }
        if let Some(x) = update.category {
            self.category = x;
        }
        if let Some(x) = update.note {
            self.note = x;
        }
        if let Some(x) = update.tags {
            self.tags = x;
        }
        if let Some(x) = update.categorized_date {
            self.categorized_date = x;
        }
        if let Some(x) = update.statement {
            self.statement = x;
        }
        if let Some(x) = update.metadata {
            self.metadata = x;
        }
        if let Some(x) = update.no_name {
            self.no_name = x;
        }

        for (key, val) in update.other_fields {
            self.other_fields.insert(key, val);
        }
    }
}

/// Represents the known columns that should be found in the transactions sheet.
/// See tiller documentation for more information about the semantic meanings of transaction
/// columns: https://help.tiller.com/en/articles/432681-transactions-sheet-columns>
#[derive(
    Default,
    Debug,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TransactionColumn {
    /// A unique ID assigned to the transaction by Tiller's systems. Critical for support
    /// troubleshooting and must not be deleted.
    #[default]
    TransactionId,
    /// The posted date (when the transaction cleared) or transaction date (when the transaction
    /// occurred). Posted date takes priority except for investment accounts.
    Date,
    /// Cleaned-up merchant information from your bank.
    Description,
    /// Transaction value where income and credits are positive; expenses and debits are negative.
    Amount,
    /// The account name as it appears on your bank's website or your custom nickname from Tiller
    /// Console.
    Account,
    /// Last four digits of the bank account number (e.g., "xxxx1102").
    AccountNumber,
    /// Financial institution name (e.g., "Bank of America").
    Institution,
    /// First day of the transaction's month, useful for pivot tables and reporting.
    Month,
    /// Sunday date of the transaction's week for weekly breakdowns.
    Week,
    /// Unmodified merchant details directly from your bank, including codes and numbers.
    FullDescription,
    /// A unique ID assigned to your accounts by Tiller's systems. Important for troubleshooting;
    /// do not delete.
    AccountId,
    /// Check number when available for checks you write.
    CheckNumber,
    /// When the transaction was added to the spreadsheet.
    DateAdded,
    /// Normalized merchant name standardizing variants (e.g., "Amazon" for multiple Amazon
    /// formats). Optional automated column.
    MerchantName,
    /// Data provider's category suggestion based on merchant knowledge. Optional automated column;
    /// not included in core templates.
    CategoryHint,
    /// User-assigned category. Non-automated by default to promote spending awareness; AutoCat
    /// available for automation.
    Category,
    /// Custom notes about specific transactions. Leveraged by Category Rollup reports.
    Note,
    /// User-defined tags for additional transaction categorization.
    Tags,
    /// Date when AutoCat automatically categorized or updated a transaction. Google Sheets Add-on
    /// column.
    CategorizedDate,
    /// For reconciling transactions to bank statements. Google Sheets Add-on column.
    Statement,
    /// Supports workflows including CSV imports. Google Sheets Add-on column.
    Metadata,
    /// My sheet has an empty column at Column A which I did not add.
    NoName,
}

serde_plain::derive_display_from_serialize!(TransactionColumn);
serde_plain::derive_fromstr_from_deserialize!(TransactionColumn);

impl TransactionColumn {
    pub fn from_header(header: impl AsRef<str>) -> Res<TransactionColumn> {
        let header_str = header.as_ref();
        match header_str {
            TRANSACTION_ID_STR => Ok(TransactionColumn::TransactionId),
            DATE_STR => Ok(TransactionColumn::Date),
            DESCRIPTION_STR => Ok(TransactionColumn::Description),
            AMOUNT_STR => Ok(TransactionColumn::Amount),
            ACCOUNT_STR => Ok(TransactionColumn::Account),
            ACCOUNT_NUMBER_STR => Ok(TransactionColumn::AccountNumber),
            INSTITUTION_STR => Ok(TransactionColumn::Institution),
            MONTH_STR => Ok(TransactionColumn::Month),
            WEEK_STR => Ok(TransactionColumn::Week),
            FULL_DESCRIPTION_STR => Ok(TransactionColumn::FullDescription),
            ACCOUNT_ID_STR => Ok(TransactionColumn::AccountId),
            CHECK_NUMBER_STR => Ok(TransactionColumn::CheckNumber),
            DATE_ADDED_STR => Ok(TransactionColumn::DateAdded),
            MERCHANT_NAME_STR => Ok(TransactionColumn::MerchantName),
            CATEGORY_HINT_STR => Ok(TransactionColumn::CategoryHint),
            CATEGORY_STR => Ok(TransactionColumn::Category),
            NOTE_STR => Ok(TransactionColumn::Note),
            TAGS_STR => Ok(TransactionColumn::Tags),
            CATEGORIZED_DATE_STR => Ok(TransactionColumn::CategorizedDate),
            STATEMENT_STR => Ok(TransactionColumn::Statement),
            METADATA_STR => Ok(TransactionColumn::Metadata),
            NO_NAME_STR => Ok(TransactionColumn::NoName),
            bad => bail!("Invalid transaction column name '{bad}'"),
        }
    }

    /// Returns the header string for this column (e.g., "Note", "Category").
    pub fn to_header(&self) -> &'static str {
        match self {
            TransactionColumn::TransactionId => TRANSACTION_ID_STR,
            TransactionColumn::Date => DATE_STR,
            TransactionColumn::Description => DESCRIPTION_STR,
            TransactionColumn::Amount => AMOUNT_STR,
            TransactionColumn::Account => ACCOUNT_STR,
            TransactionColumn::AccountNumber => ACCOUNT_NUMBER_STR,
            TransactionColumn::Institution => INSTITUTION_STR,
            TransactionColumn::Month => MONTH_STR,
            TransactionColumn::Week => WEEK_STR,
            TransactionColumn::FullDescription => FULL_DESCRIPTION_STR,
            TransactionColumn::AccountId => ACCOUNT_ID_STR,
            TransactionColumn::CheckNumber => CHECK_NUMBER_STR,
            TransactionColumn::DateAdded => DATE_ADDED_STR,
            TransactionColumn::MerchantName => MERCHANT_NAME_STR,
            TransactionColumn::CategoryHint => CATEGORY_HINT_STR,
            TransactionColumn::Category => CATEGORY_STR,
            TransactionColumn::Note => NOTE_STR,
            TransactionColumn::Tags => TAGS_STR,
            TransactionColumn::CategorizedDate => CATEGORIZED_DATE_STR,
            TransactionColumn::Statement => STATEMENT_STR,
            TransactionColumn::Metadata => METADATA_STR,
            TransactionColumn::NoName => NO_NAME_STR,
        }
    }
}

impl AsRef<str> for TransactionColumn {
    fn as_ref(&self) -> &str {
        self.to_header()
    }
}

/// The fields to update in a transaction row. Only set values will be changed, unset values will
/// not be changed.
///
/// See tiller documentation for more information about the semantic meanings of transaction
/// columns: https://help.tiller.com/en/articles/432681-transactions-sheet-columns>
#[derive(Debug, Default, Clone, Parser, Serialize, Deserialize, JsonSchema)]
pub struct TransactionUpdates {
    /// The posted date (when the transaction cleared) or transaction date (when the transaction
    /// occurred). Posted date takes priority except for investment accounts.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub date: Option<String>,

    /// Cleaned-up merchant information from your bank.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description: Option<String>,

    /// Transaction value where income and credits are positive; expenses and debits are negative.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub amount: Option<Amount>,

    /// The account name as it appears on your bank's website or your custom nickname from Tiller
    /// Console.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account: Option<String>,

    /// Last four digits of the bank account number (e.g., "xxxx1102").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account_number: Option<String>,

    /// Financial institution name (e.g., "Bank of America").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub institution: Option<String>,

    /// First day of the transaction's month, useful for pivot tables and reporting.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub month: Option<String>,

    /// Sunday date of the transaction's week for weekly breakdowns.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub week: Option<String>,

    /// Unmodified merchant details directly from your bank, including codes and numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub full_description: Option<String>,

    /// A unique ID assigned to your accounts by Tiller's systems. Important for troubleshooting;
    /// do not delete.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account_id: Option<String>,

    /// Check number when available for checks you write.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub check_number: Option<String>,

    /// When the transaction was added to the spreadsheet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub date_added: Option<String>,

    /// Normalized merchant name standardizing variants (e.g., "Amazon" for multiple Amazon
    /// formats). Optional automated column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub merchant_name: Option<String>,

    /// Data provider's category suggestion based on merchant knowledge. Optional automated column;
    /// not included in core templates.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub category_hint: Option<String>,

    /// User-assigned category. Non-automated by default to promote spending awareness; AutoCat
    /// available for automation.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub category: Option<String>,

    /// Custom notes about specific transactions. Leveraged by Category Rollup reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub note: Option<String>,

    /// User-defined tags for additional transaction categorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub tags: Option<String>,

    /// Date when AutoCat automatically categorized or updated a transaction. Google Sheets Add-on
    /// column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub categorized_date: Option<String>,

    /// For reconciling transactions to bank statements. Google Sheets Add-on column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub statement: Option<String>,

    /// Supports workflows including CSV imports. Google Sheets Add-on column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub metadata: Option<String>,

    /// Empty column that may appear at Column A.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub no_name: Option<String>,

    /// Custom columns not part of the standard Tiller schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[arg(long = "other-field", value_parser = utils::parse_key_val)]
    pub other_fields: BTreeMap<String, String>,
}

pub(super) const TRANSACTION_ID_STR: &str = "Transaction ID";
pub(super) const DATE_STR: &str = "Date";
pub(super) const DESCRIPTION_STR: &str = "Description";
pub(super) const AMOUNT_STR: &str = "Amount";
pub(super) const ACCOUNT_STR: &str = "Account";
pub(super) const ACCOUNT_NUMBER_STR: &str = "Account #";
pub(super) const INSTITUTION_STR: &str = "Institution";
pub(super) const MONTH_STR: &str = "Month";
pub(super) const WEEK_STR: &str = "Week";
pub(super) const FULL_DESCRIPTION_STR: &str = "Full Description";
pub(super) const ACCOUNT_ID_STR: &str = "Account ID";
pub(super) const CHECK_NUMBER_STR: &str = "Check Number";
pub(super) const DATE_ADDED_STR: &str = "Date Added";
pub(super) const MERCHANT_NAME_STR: &str = "Merchant Name";
pub(super) const CATEGORY_HINT_STR: &str = "Category Hint";
pub(super) const CATEGORY_STR: &str = "Category";
pub(super) const NOTE_STR: &str = "Note";
pub(super) const TAGS_STR: &str = "Tags";
pub(super) const CATEGORIZED_DATE_STR: &str = "Categorized Date";
pub(super) const STATEMENT_STR: &str = "Statement";
pub(super) const METADATA_STR: &str = "Metadata";
pub(super) const NO_NAME_STR: &str = "";
