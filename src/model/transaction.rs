use crate::model::mapping::Mapping;
use crate::model::{Amount, RowCol};
use crate::Result;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

/// Represents the transaction data from a Transactions sheet, including the header mapping.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Transactions {
    mapping: Mapping,
    data: Vec<Transaction>,
    /// Maps (row_index, column_index) -> formula for cells that contain formulas.
    /// Stored exactly as returned by the Google Sheets API.
    formulas: BTreeMap<RowCol, String>,
}

impl Transactions {
    pub fn new<S, R>(
        sheet_data: impl IntoIterator<Item = R>,
        formula_data: impl IntoIterator<Item = R>,
    ) -> Result<Self>
    where
        S: Into<String>,
        R: IntoIterator<Item = S>,
    {
        let mut rows = sheet_data.into_iter();
        let mapping = match rows.next() {
            Some(header_row) => Mapping::new(header_row.into_iter())?,
            None => bail!("An empty data set cannot be parsed into a Transactions object"),
        };

        let len = mapping.len();

        // Convert formula data to Vec<Vec<String>> for comparison
        let formula_rows: Vec<Vec<String>> = formula_data
            .into_iter()
            .map(|row| row.into_iter().map(|s| s.into()).collect())
            .collect();

        // Detect formulas by comparing values vs formulas
        let mut formulas = BTreeMap::new();
        let mut transactions = Vec::new();

        for (row_ix, row) in rows.enumerate() {
            let values: Vec<String> = row.into_iter().map(|s| s.into()).collect();
            if values.len() > len {
                bail!(
                    "A row longer than the headers list was encountered at row {}",
                    row_ix + 2
                );
            }

            // Compare with formula row to detect formulas (row_ix+1 because we skipped header)
            if let Some(formula_row) = formula_rows.get(row_ix + 1) {
                for (col_ix, value) in values.iter().enumerate() {
                    if let Some(formula) = formula_row.get(col_ix) {
                        // If formula differs from value, it's a formula cell
                        if formula != value {
                            formulas.insert(RowCol::new(row_ix, col_ix), formula.clone());
                        }
                    }
                }
            }

            transactions.push(Transaction::new_with_sheet_headers(
                mapping.headers(),
                values,
            )?);
        }
        Ok(Self {
            mapping,
            data: transactions,
            formulas,
        })
    }
}

/// Represents a single row from the Transactions sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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
    pub(crate) categorized_date: String,
    pub(crate) statement: String,
    pub(crate) metadata: String,
    pub(crate) no_name: String,
    pub(crate) other_fields: BTreeMap<String, String>,
}

impl Transaction {
    pub fn new_with_sheet_headers<S1, S2, I>(headers: &[S1], values: I) -> Result<Self>
    where
        S1: AsRef<str>,
        S2: Into<String>,
        I: IntoIterator<Item = S2>,
    {
        let mut transaction = Transaction::default();
        for (ix, value) in values.into_iter().map(|s| s.into()).enumerate() {
            let header = headers
                .get(ix)
                .with_context(|| format!("No header found for column index {ix}"))?
                .as_ref();
            transaction.set_with_header(header, value)?;
        }
        Ok(transaction)
    }

    pub fn new_with_sql_columns<S1, S2>(_column: &[S1], _values: &[S2]) -> Result<Self>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        todo!();
    }

    pub fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Result<()>
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

    pub fn set_by_column_name() -> Result<Self> {
        todo!()
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
    pub fn from_header(header: impl AsRef<str>) -> Result<TransactionColumn> {
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

// TODO: remove this if it continues to go unused
impl TransactionColumn {
    fn _as_header_str(&self) -> &str {
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

pub(super) const TRANSACTION_ID_STR: &str = "Transaction ID";
pub(super) const _TRANSACTION_ID_COL: &str = "transaction_id";
pub(super) const _TRANSACTION_ID_IDX: usize = 0;

pub(super) const DATE_STR: &str = "Date";
pub(super) const _DATE_COL: &str = "date";
pub(super) const _DATE_IDX: usize = 1;

pub(super) const DESCRIPTION_STR: &str = "Description";
pub(super) const _DESCRIPTION_COL: &str = "description";
pub(super) const _DESCRIPTION_IDX: usize = 2;

pub(super) const AMOUNT_STR: &str = "Amount";
pub(super) const _AMOUNT_COL: &str = "amount";
pub(super) const _AMOUNT_IDX: usize = 3;

pub(super) const ACCOUNT_STR: &str = "Account";
pub(super) const _ACCOUNT_COL: &str = "account";
pub(super) const _ACCOUNT_IDX: usize = 4;

pub(super) const ACCOUNT_NUMBER_STR: &str = "Account #";
pub(super) const _ACCOUNT_NUMBER_COL: &str = "account_number";
pub(super) const _ACCOUNT_NUMBER_IDX: usize = 5;

pub(super) const INSTITUTION_STR: &str = "Institution";
pub(super) const _INSTITUTION_COL: &str = "institution";
pub(super) const _INSTITUTION_IDX: usize = 6;

pub(super) const MONTH_STR: &str = "Month";
pub(super) const _MONTH_COL: &str = "month";
pub(super) const _MONTH_IDX: usize = 7;

pub(super) const WEEK_STR: &str = "Week";
pub(super) const _WEEK_COL: &str = "week";
pub(super) const _WEEK_IDX: usize = 8;

pub(super) const FULL_DESCRIPTION_STR: &str = "Full Description";
pub(super) const _FULL_DESCRIPTION_COL: &str = "full_description";
pub(super) const _FULL_DESCRIPTION_IDX: usize = 9;

pub(super) const ACCOUNT_ID_STR: &str = "Account ID";
pub(super) const _ACCOUNT_ID_COL: &str = "account_id";
pub(super) const _ACCOUNT_ID_IDX: usize = 10;

pub(super) const CHECK_NUMBER_STR: &str = "Check Number";
pub(super) const _CHECK_NUMBER_COL: &str = "check_number";
pub(super) const _CHECK_NUMBER_IDX: usize = 11;

pub(super) const DATE_ADDED_STR: &str = "Date Added";
pub(super) const _DATE_ADDED_COL: &str = "date_added";
pub(super) const _DATE_ADDED_IDX: usize = 12;

pub(super) const MERCHANT_NAME_STR: &str = "Merchant Name";
pub(super) const _MERCHANT_NAME_COL: &str = "merchant_name";
pub(super) const _MERCHANT_NAME_IDX: usize = 13;

pub(super) const CATEGORY_HINT_STR: &str = "Category Hint";
pub(super) const _CATEGORY_HINT_COL: &str = "category_hint";
pub(super) const _CATEGORY_HINT_IDX: usize = 14;

pub(super) const CATEGORY_STR: &str = "Category";
pub(super) const _CATEGORY_COL: &str = "category";
pub(super) const _CATEGORY_IDX: usize = 15;

pub(super) const NOTE_STR: &str = "Note";
pub(super) const _NOTE_COL: &str = "note";
pub(super) const _NOTE_IDX: usize = 16;

pub(super) const TAGS_STR: &str = "Tags";
pub(super) const _TAGS_COL: &str = "tags";
pub(super) const _TAGS_IDX: usize = 17;

pub(super) const CATEGORIZED_DATE_STR: &str = "Categorized Date";
pub(super) const _CATEGORIZED_DATE_COL: &str = "categorized_date";
pub(super) const _CATEGORIZED_DATE_IDX: usize = 18;

pub(super) const STATEMENT_STR: &str = "Statement";
pub(super) const _STATEMENT_COL: &str = "statement";
pub(super) const _STATEMENT_IDX: usize = 19;

pub(super) const METADATA_STR: &str = "Metadata";
pub(super) const _METADATA_COL: &str = "metadata";
pub(super) const _METADATA_IDX: usize = 20;

pub(super) const NO_NAME_STR: &str = "";
pub(super) const _NO_NAME_COL: &str = "no_name";
pub(super) const _NO_NAME_IDX: usize = 21;

pub(super) const _TRANSACTION_COL_COUNT: usize = 22;
