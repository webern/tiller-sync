use crate::error::Res;
use crate::model::items::{Item, Items};
use crate::model::Amount;
use crate::utils;
use anyhow::{bail, Context};
use clap::Parser;
use schemars::JsonSchema;
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

impl AutoCat {
    /// Set any of the fields on `self` that are set in `update`.
    pub fn merge_updates(&mut self, update: AutoCatUpdates) {
        if let Some(x) = update.category {
            self.category = x;
        }
        if let Some(x) = update.description {
            self.description = x;
        }
        if let Some(x) = update.description_contains {
            self.description_contains = x;
        }
        if let Some(x) = update.account_contains {
            self.account_contains = x;
        }
        if let Some(x) = update.institution_contains {
            self.institution_contains = x;
        }
        if let Some(x) = update.amount_min {
            self.amount_min = Some(x);
        }
        if let Some(x) = update.amount_max {
            self.amount_max = Some(x);
        }
        if let Some(x) = update.amount_equals {
            self.amount_equals = Some(x);
        }
        if let Some(x) = update.description_equals {
            self.description_equals = x;
        }
        if let Some(x) = update.description_full {
            self.description_full = x;
        }
        if let Some(x) = update.full_description_contains {
            self.full_description_contains = x;
        }
        if let Some(x) = update.amount_contains {
            self.amount_contains = x;
        }

        for (key, val) in update.other_fields {
            self.other_fields.insert(key, val);
        }
    }
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

/// The fields to update in an AutoCat rule. Only set values will be changed, unset values will
/// not be changed.
///
/// See tiller documentation for more information about AutoCat:
/// <https://help.tiller.com/en/articles/3792984-autocat-for-google-sheets>
#[derive(Debug, Default, Clone, Parser, Serialize, Deserialize, JsonSchema)]
pub struct AutoCatUpdates {
    /// The category to assign when this rule matches. This is an override column - when filter
    /// conditions match, this category value gets applied to matching transactions.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub category: Option<String>,

    /// Override column to standardize or clean up transaction descriptions. For example, replace
    /// "Seattle Starbucks store 1234" with simply "Starbucks".
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description: Option<String>,

    /// Filter criteria: searches the Description column for matching text (case-insensitive).
    /// Supports multiple keywords wrapped in quotes and separated by commas (OR-ed together).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description_contains: Option<String>,

    /// Filter criteria: searches the Account column for matching text to narrow rule application.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account_contains: Option<String>,

    /// Filter criteria: searches the Institution column for matching text to narrow rule
    /// application.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub institution_contains: Option<String>,

    /// Filter criteria: minimum transaction amount (absolute value). Use with Amount Max to set
    /// a range. For negative amounts (expenses), set Amount Polarity to "Negative".
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, value_parser = utils::parse_amount)]
    pub amount_min: Option<Amount>,

    /// Filter criteria: maximum transaction amount (absolute value). Use with Amount Min to set
    /// a range. For negative amounts (expenses), set Amount Polarity to "Negative".
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, value_parser = utils::parse_amount)]
    pub amount_max: Option<Amount>,

    /// Filter criteria: exact amount to match.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, value_parser = utils::parse_amount)]
    pub amount_equals: Option<Amount>,

    /// Filter criteria: exact match for the Description column (more specific than "contains").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description_equals: Option<String>,

    /// Override column for the full/raw description field.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description_full: Option<String>,

    /// Filter criteria: searches the Full Description column for matching text.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub full_description_contains: Option<String>,

    /// Filter criteria: searches the Amount column as text for matching patterns.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub amount_contains: Option<String>,

    /// Custom columns not part of the standard Tiller schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[arg(long = "other-field", value_parser = utils::parse_key_val)]
    pub other_fields: BTreeMap<String, String>,
}
