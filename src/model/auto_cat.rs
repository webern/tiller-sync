use crate::error::Res;
use crate::model::items::{Item, Items};
use crate::model::Amount;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;

/// Represents the AutoCat data from an AutoCat sheet, including the header mapping.
pub type AutoCats = Items<AutoCat>;

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
    /// Row position from last sync down (0-indexed); None for locally-added rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) original_order: Option<u64>,
}

impl Item for AutoCat {
    fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Res<()>
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

    fn get_by_header(&self, header: &str) -> String {
        match AutoCatColumn::from_header(header) {
            Ok(col) => match col {
                AutoCatColumn::Category => self.category.clone(),
                AutoCatColumn::Description => self.description.clone(),
                AutoCatColumn::DescriptionContains => self.description_contains.clone(),
                AutoCatColumn::AccountContains => self.account_contains.clone(),
                AutoCatColumn::InstitutionContains => self.institution_contains.clone(),
                AutoCatColumn::AmountMin => optional_amount_to_string(&self.amount_min),
                AutoCatColumn::AmountMax => optional_amount_to_string(&self.amount_max),
                AutoCatColumn::AmountEquals => optional_amount_to_string(&self.amount_equals),
                AutoCatColumn::DescriptionEquals => self.description_equals.clone(),
                AutoCatColumn::DescriptionFull => self.description_full.clone(),
                AutoCatColumn::FullDescriptionContains => self.full_description_contains.clone(),
                AutoCatColumn::AmountContains => self.amount_contains.clone(),
            },
            Err(_) => self.other_fields.get(header).cloned().unwrap_or_default(),
        }
    }

    fn set_original_order(&mut self, original_order: u64) {
        self.original_order = Some(original_order)
    }

    fn get_original_order(&self) -> Option<u64> {
        self.original_order
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
    pub fn from_header(header: impl AsRef<str>) -> Res<AutoCatColumn> {
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

/// Parses an optional amount value
fn parse_optional_amount(s: &str) -> Res<Option<Amount>> {
    if s.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        Amount::from_str(s).context(format!("Failed to parse amount value: {s}"))?,
    ))
}

/// Converts an optional amount to a string for sheet output
fn optional_amount_to_string(amount: &Option<Amount>) -> String {
    match amount {
        Some(a) => a.to_string(),
        None => String::new(),
    }
}

pub(super) const CATEGORY_STR: &str = "Category";
pub(super) const DESCRIPTION_STR: &str = "Description";
pub(super) const DESCRIPTION_CONTAINS_STR: &str = "Description Contains";
pub(super) const ACCOUNT_CONTAINS_STR: &str = "Account Contains";
pub(super) const INSTITUTION_CONTAINS_STR: &str = "Institution Contains";
pub(super) const AMOUNT_MIN_STR: &str = "Amount Min";
pub(super) const AMOUNT_MAX_STR: &str = "Amount Max";
pub(super) const AMOUNT_EQUALS_STR: &str = "Amount Equals";
pub(super) const DESCRIPTION_EQUALS_STR: &str = "Description Equals";
pub(super) const DESCRIPTION_FULL_STR: &str = "Description Full";
pub(super) const FULL_DESCRIPTION_CONTAINS_STR: &str = "Full Description Contains";
pub(super) const AMOUNT_CONTAINS_STR: &str = "Amount Contains";
