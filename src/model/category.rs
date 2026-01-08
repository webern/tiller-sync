use crate::error::Res;
use crate::model::items::{Item, Items};
use crate::utils;
use anyhow::bail;
use clap::Parser;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Represents the category data from a Categories sheet, including the header mapping.
pub type Categories = Items<Category>;

/// Represents a single row from the Category sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Category {
    // TODO: make these private again
    pub(crate) category: String,
    pub(crate) category_group: String,
    #[serde(rename = "type")]
    pub(crate) r#type: String,
    pub(crate) hide_from_reports: String,
    pub(crate) other_fields: BTreeMap<String, String>,
    /// Row position from last sync down (0-indexed); None for locally-added rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) original_order: Option<u64>,
}

impl Category {
    /// Set any of the fields on `self` that are set in `update`.
    pub fn merge_updates(&mut self, update: CategoryUpdates) {
        if let Some(x) = update.category {
            self.category = x;
        }
        if let Some(x) = update.group {
            self.category_group = x;
        }
        if let Some(x) = update.r#type {
            self.r#type = x;
        }
        if let Some(x) = update.hide_from_reports {
            self.hide_from_reports = x;
        }

        for (key, val) in update.other_fields {
            self.other_fields.insert(key, val);
        }
    }
}

impl Item for Category {
    fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Res<()>
    where
        S1: AsRef<str>,
        S2: Into<String>,
    {
        let header = header.as_ref();
        let value = value.into();

        match CategoryColumn::from_header(header) {
            Ok(col) => match col {
                CategoryColumn::Category => self.category = value,
                CategoryColumn::Group => self.category_group = value,
                CategoryColumn::Type => self.r#type = value,
                CategoryColumn::HideFromReports => self.hide_from_reports = value,
            },
            Err(_) => {
                let _ = self.other_fields.insert(header.to_string(), value);
            }
        }

        Ok(())
    }

    fn get_by_header(&self, header: &str) -> String {
        match CategoryColumn::from_header(header) {
            Ok(col) => match col {
                CategoryColumn::Category => self.category.clone(),
                CategoryColumn::Group => self.category_group.clone(),
                CategoryColumn::Type => self.r#type.clone(),
                CategoryColumn::HideFromReports => self.hide_from_reports.clone(),
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

/// Represents the known columns that should be found in the categories sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryColumn {
    #[default]
    Category,
    Group,
    #[serde(rename = "type")]
    Type,
    HideFromReports,
}

serde_plain::derive_display_from_serialize!(CategoryColumn);
serde_plain::derive_fromstr_from_deserialize!(CategoryColumn);

impl CategoryColumn {
    pub fn from_header(header: impl AsRef<str>) -> Res<CategoryColumn> {
        let header_str = header.as_ref();
        match header_str {
            CATEGORY_STR => Ok(CategoryColumn::Category),
            GROUP_STR => Ok(CategoryColumn::Group),
            TYPE_STR => Ok(CategoryColumn::Type),
            HIDE_FROM_REPORTS_STR => Ok(CategoryColumn::HideFromReports),
            bad => bail!("Invalid category column name '{bad}'"),
        }
    }
}

pub(super) const CATEGORY_STR: &str = "Category";
pub(super) const GROUP_STR: &str = "Group";
pub(super) const TYPE_STR: &str = "Type";
pub(super) const HIDE_FROM_REPORTS_STR: &str = "Hide from Reports";

/// The fields to update in a category row. Only set values will be changed, unset values will
/// not be changed.
///
/// See tiller documentation for more information about the Categories sheet:
/// <https://help.tiller.com/en/articles/3250769-customizing-categories>
#[derive(Debug, Default, Clone, Parser, Serialize, Deserialize, JsonSchema)]
pub struct CategoryUpdates {
    /// The new name for the category. Use this to rename a category. Due to `ON UPDATE CASCADE`
    /// foreign key constraints, renaming a category automatically updates all references in
    /// transactions and autocat rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub category: Option<String>,

    /// The group this category belongs to. Groups organize related categories together for
    /// reporting purposes (e.g., "Food", "Transportation", "Housing"). All categories should have
    /// a Group assigned.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub group: Option<String>,

    /// The type classification for this category. Common types include "Expense", "Income", and
    /// "Transfer". All categories should have a Type assigned.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, name = "type")]
    pub r#type: Option<String>,

    /// Controls visibility in reports. Set to "Hide" to exclude this category from reports.
    /// This is useful for categories like credit card payments or internal transfers that you
    /// don't want appearing in spending reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub hide_from_reports: Option<String>,

    /// Custom columns not part of the standard Tiller schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[arg(long = "other-field", value_parser = utils::parse_key_val)]
    pub other_fields: BTreeMap<String, String>,
}
