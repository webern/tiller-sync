use crate::model::items::{Item, Items};
use crate::Result;
use anyhow::bail;
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

impl Item for Category {
    fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Result<()>
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
    pub fn from_header(header: impl AsRef<str>) -> Result<CategoryColumn> {
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
