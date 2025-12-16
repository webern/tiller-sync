use crate::model::mapping::Mapping;
use crate::model::{Amount, RowCol};
use crate::Result;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

/// Represents the AutoCat data from an AutoCat sheet, including the header mapping.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AutoCats {
    mapping: Mapping,
    data: Vec<AutoCat>,
    /// Maps (row_index, column_index) -> formula for cells that contain formulas.
    /// Stored exactly as returned by the Google Sheets API.
    formulas: BTreeMap<RowCol, String>,
}

impl AutoCats {
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
            None => bail!("An empty data set cannot be parsed into an AutoCats object"),
        };

        let len = mapping.len();

        // Convert formula data to Vec<Vec<String>> for comparison
        let formula_rows: Vec<Vec<String>> = formula_data
            .into_iter()
            .map(|row| row.into_iter().map(|s| s.into()).collect())
            .collect();

        // Detect formulas by comparing values vs formulas
        let mut formulas = BTreeMap::new();
        let mut auto_cats = Vec::new();

        for (row_ix, row) in rows.enumerate() {
            let values: Vec<String> = row.into_iter().map(|s| s.into()).collect();
            if values.is_empty() {
                continue; // Skip empty rows
            }
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

            auto_cats.push(AutoCat::new_with_sheet_headers(mapping.headers(), values)?);
        }
        Ok(Self {
            mapping,
            data: auto_cats,
            formulas,
        })
    }

    pub fn data(&self) -> &Vec<AutoCat> {
        &self.data
    }

    /// Creates an AutoCats object from a Vec of AutoCat items.
    /// Used when loading from the database where we don't have sheet headers.
    pub(crate) fn _from_data(data: Vec<AutoCat>) -> Self {
        Self {
            mapping: Mapping::default(),
            data,
            formulas: BTreeMap::new(),
        }
    }
}

/// Represents a single row from the AutoCat sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AutoCat {
    pub(crate) category: String,
    pub(crate) description: String,
    pub(crate) description_contains: String,
    pub(crate) account_contains: String,
    pub(crate) institution_contains: String,
    pub(crate) amount_min: Option<Amount>,
    pub(crate) amount_max: Option<Amount>,
    pub(crate) amount_equals: Option<Amount>,
    pub(crate) description_equals: String,
    pub(crate) description_full: String,
    pub(crate) full_description_contains: String,
    pub(crate) amount_contains: String,
    pub(crate) other_fields: BTreeMap<String, String>,
}

impl AutoCat {
    pub fn new_with_sheet_headers<S1, S2, I>(headers: &[S1], values: I) -> Result<Self>
    where
        S1: AsRef<str>,
        S2: Into<String>,
        I: IntoIterator<Item = S2>,
    {
        let mut auto_cat = AutoCat::default();
        for (ix, value) in values.into_iter().map(|s| s.into()).enumerate() {
            let header = headers
                .get(ix)
                .with_context(|| format!("No header found for column index {ix}"))?
                .as_ref();
            auto_cat.set_with_header(header, value)?;
        }
        Ok(auto_cat)
    }

    pub fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Result<()>
    where
        S1: AsRef<str>,
        S2: Into<String>,
    {
        let header = header.as_ref();
        let value = value.into();

        match AutoCatColumn::from_header(header) {
            Ok(col) => match col {
                AutoCatColumn::Category => self.category = value,
                AutoCatColumn::Description => self.description = value,
                AutoCatColumn::DescriptionContains => self.description_contains = value,
                AutoCatColumn::AccountContains => self.account_contains = value,
                AutoCatColumn::InstitutionContains => self.institution_contains = value,
                AutoCatColumn::AmountMin => self.amount_min = parse_optional_amount(&value)?,
                AutoCatColumn::AmountMax => self.amount_max = parse_optional_amount(&value)?,
                AutoCatColumn::AmountEquals => self.amount_equals = parse_optional_amount(&value)?,
                AutoCatColumn::DescriptionEquals => self.description_equals = value,
                AutoCatColumn::DescriptionFull => self.description_full = value,
                AutoCatColumn::FullDescriptionContains => self.full_description_contains = value,
                AutoCatColumn::AmountContains => self.amount_contains = value,
            },
            Err(_) => {
                let _ = self.other_fields.insert(header.to_string(), value);
            }
        }

        Ok(())
    }
}

/// Represents the known columns that should be found in the AutoCat sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoCatColumn {
    #[default]
    Category,
    Description,
    DescriptionContains,
    AccountContains,
    InstitutionContains,
    AmountMin,
    AmountMax,
    AmountEquals,
    DescriptionEquals,
    DescriptionFull,
    FullDescriptionContains,
    AmountContains,
}

serde_plain::derive_display_from_serialize!(AutoCatColumn);
serde_plain::derive_fromstr_from_deserialize!(AutoCatColumn);

impl AutoCatColumn {
    pub fn from_header(header: impl AsRef<str>) -> Result<AutoCatColumn> {
        let header_str = header.as_ref();
        match header_str {
            CATEGORY_STR => Ok(AutoCatColumn::Category),
            DESCRIPTION_STR => Ok(AutoCatColumn::Description),
            DESCRIPTION_CONTAINS_STR => Ok(AutoCatColumn::DescriptionContains),
            ACCOUNT_CONTAINS_STR => Ok(AutoCatColumn::AccountContains),
            INSTITUTION_CONTAINS_STR => Ok(AutoCatColumn::InstitutionContains),
            AMOUNT_MIN_STR => Ok(AutoCatColumn::AmountMin),
            AMOUNT_MAX_STR => Ok(AutoCatColumn::AmountMax),
            AMOUNT_EQUALS_STR => Ok(AutoCatColumn::AmountEquals),
            DESCRIPTION_EQUALS_STR => Ok(AutoCatColumn::DescriptionEquals),
            DESCRIPTION_FULL_STR => Ok(AutoCatColumn::DescriptionFull),
            FULL_DESCRIPTION_CONTAINS_STR => Ok(AutoCatColumn::FullDescriptionContains),
            AMOUNT_CONTAINS_STR => Ok(AutoCatColumn::AmountContains),
            bad => bail!("Invalid AutoCat column name '{bad}'"),
        }
    }
}

// TODO: perhaps remove this if unused in the future
impl AutoCatColumn {
    fn _as_header_str(&self) -> &str {
        match self {
            AutoCatColumn::Category => CATEGORY_STR,
            AutoCatColumn::Description => DESCRIPTION_STR,
            AutoCatColumn::DescriptionContains => DESCRIPTION_CONTAINS_STR,
            AutoCatColumn::AccountContains => ACCOUNT_CONTAINS_STR,
            AutoCatColumn::InstitutionContains => INSTITUTION_CONTAINS_STR,
            AutoCatColumn::AmountMin => AMOUNT_MIN_STR,
            AutoCatColumn::AmountMax => AMOUNT_MAX_STR,
            AutoCatColumn::AmountEquals => AMOUNT_EQUALS_STR,
            AutoCatColumn::DescriptionEquals => DESCRIPTION_EQUALS_STR,
            AutoCatColumn::DescriptionFull => DESCRIPTION_FULL_STR,
            AutoCatColumn::FullDescriptionContains => FULL_DESCRIPTION_CONTAINS_STR,
            AutoCatColumn::AmountContains => AMOUNT_CONTAINS_STR,
        }
    }
}

/// Parses an optional amount value
fn parse_optional_amount(s: &str) -> Result<Option<Amount>> {
    if s.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        Amount::from_str(s).context(format!("Failed to parse amount value: {s}"))?,
    ))
}

pub(super) const CATEGORY_STR: &str = "Category";
pub(super) const _CATEGORY_COL: &str = "category";
pub(super) const _CATEGORY_IDX: usize = 0;

pub(super) const DESCRIPTION_STR: &str = "Description";
pub(super) const _DESCRIPTION_COL: &str = "description";
pub(super) const _DESCRIPTION_IDX: usize = 1;

pub(super) const DESCRIPTION_CONTAINS_STR: &str = "Description Contains";
pub(super) const _DESCRIPTION_CONTAINS_COL: &str = "description_contains";
pub(super) const _DESCRIPTION_CONTAINS_IDX: usize = 2;

pub(super) const ACCOUNT_CONTAINS_STR: &str = "Account Contains";
pub(super) const _ACCOUNT_CONTAINS_COL: &str = "account_contains";
pub(super) const _ACCOUNT_CONTAINS_IDX: usize = 3;

pub(super) const INSTITUTION_CONTAINS_STR: &str = "Institution Contains";
pub(super) const _INSTITUTION_CONTAINS_COL: &str = "institution_contains";
pub(super) const _INSTITUTION_CONTAINS_IDX: usize = 4;

pub(super) const AMOUNT_MIN_STR: &str = "Amount Min";
pub(super) const _AMOUNT_MIN_COL: &str = "amount_min";
pub(super) const _AMOUNT_MIN_IDX: usize = 5;

pub(super) const AMOUNT_MAX_STR: &str = "Amount Max";
pub(super) const _AMOUNT_MAX_COL: &str = "amount_max";
pub(super) const _AMOUNT_MAX_IDX: usize = 6;

pub(super) const AMOUNT_EQUALS_STR: &str = "Amount Equals";
pub(super) const _AMOUNT_EQUALS_COL: &str = "amount_equals";
pub(super) const _AMOUNT_EQUALS_IDX: usize = 7;

pub(super) const DESCRIPTION_EQUALS_STR: &str = "Description Equals";
pub(super) const _DESCRIPTION_EQUALS_COL: &str = "description_equals";
pub(super) const _DESCRIPTION_EQUALS_IDX: usize = 8;

pub(super) const DESCRIPTION_FULL_STR: &str = "Description Full";
pub(super) const _DESCRIPTION_FULL_COL: &str = "description_full";
pub(super) const _DESCRIPTION_FULL_IDX: usize = 9;

pub(super) const FULL_DESCRIPTION_CONTAINS_STR: &str = "Full Description Contains";
pub(super) const _FULL_DESCRIPTION_CONTAINS_COL: &str = "full_description_contains";
pub(super) const _FULL_DESCRIPTION_CONTAINS_IDX: usize = 10;

pub(super) const AMOUNT_CONTAINS_STR: &str = "Amount Contains";
pub(super) const _AMOUNT_CONTAINS_COL: &str = "amount_contains";
pub(super) const _AMOUNT_CONTAINS_IDX: usize = 11;

pub(super) const _AUTO_CAT_COL_COUNT: usize = 12;
