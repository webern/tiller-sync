use crate::error::Res;
use crate::model::items::{Item, Items};
use crate::model::Amount;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

/// Represents the transaction data from a Transactions sheet, including the header mapping.
pub type Transactions = Items<Transaction>;

/// Represents a single row from the Transactions sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Transaction {
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
    pub(crate) categorized_date: String,
    pub(crate) statement: String,
    pub(crate) metadata: String,
    pub(crate) no_name: String,
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

/// Represents the known columns that should be found in the transactions sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionColumn {
    #[default]
    TransactionId,
    Date,
    Description,
    Amount,
    Account,
    AccountNumber,
    Institution,
    Month,
    Week,
    FullDescription,
    AccountId,
    CheckNumber,
    DateAdded,
    MerchantName,
    CategoryHint,
    Category,
    Note,
    Tags,
    CategorizedDate,
    Statement,
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
