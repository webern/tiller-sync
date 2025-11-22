use crate::model::mapping::{AsHeader, Mapping};
use crate::Result;
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the category data from a Categories sheet, including the header mapping.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Categories {
    mapping: Mapping,
    data: Vec<Category>,
}

impl Categories {
    pub fn new<S, R>(sheet_data: impl IntoIterator<Item = R>) -> Result<Self>
    where
        S: Into<String>,
        R: IntoIterator<Item = S>,
    {
        let mut rows = sheet_data.into_iter();
        let mapping = match rows.next() {
            Some(header_row) => Mapping::new(header_row.into_iter())?,
            None => bail!("An empty data set cannot be parsed into a Categories object"),
        };

        let len = mapping.len();

        let mut categories = Vec::new();
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
            categories.push(Category::new_with_sheet_headers(mapping.headers(), values)?);
        }
        Ok(Self {
            mapping,
            data: categories,
        })
    }
}

/// Represents a single row from the Category sheet.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Category {
    // TODO: make these private again
    pub(crate) category: String,
    pub(crate) group: String,
    #[serde(rename = "type")]
    pub(crate) _type: String,
    pub(crate) hide_from_reports: String,
    pub(crate) other_fields: HashMap<String, String>,
}

impl Category {
    pub fn new_with_sheet_headers<S1, S2, I>(headers: &[S1], values: I) -> Result<Self>
    where
        S1: AsRef<str>,
        S2: Into<String>,
        I: IntoIterator<Item = S2>,
    {
        let mut category = Category::default();
        for (ix, value) in values.into_iter().map(|s| s.into()).enumerate() {
            let header = headers
                .get(ix)
                .with_context(|| format!("No header found for column index {ix}"))?
                .as_ref();
            category.set_with_header(header, value)?;
        }
        Ok(category)
    }

    pub fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Result<()>
    where
        S1: AsRef<str>,
        S2: Into<String>,
    {
        let header = header.as_ref();
        let value = value.into();

        match CategoryColumn::from_header(header) {
            Ok(col) => match col {
                CategoryColumn::Category => self.category = value,
                CategoryColumn::Group => self.group = value,
                CategoryColumn::Type => self._type = value,
                CategoryColumn::HideFromReports => self.hide_from_reports = value,
            },
            Err(_) => {
                let _ = self.other_fields.insert(header.to_string(), value);
            }
        }

        Ok(())
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

impl AsHeader for CategoryColumn {
    fn _as_header_str(&self) -> &str {
        match self {
            CategoryColumn::Category => CATEGORY_STR,
            CategoryColumn::Group => GROUP_STR,
            CategoryColumn::Type => TYPE_STR,
            CategoryColumn::HideFromReports => HIDE_FROM_REPORTS_STR,
        }
    }
}

pub(super) const CATEGORY_STR: &str = "Category";
pub(super) const _CATEGORY_COL: &str = "category";
pub(super) const _CATEGORY_IDX: usize = 0;

pub(super) const GROUP_STR: &str = "Group";
pub(super) const _GROUP_COL: &str = "group";
pub(super) const _GROUP_IDX: usize = 1;

pub(super) const TYPE_STR: &str = "Type";
pub(super) const _TYPE_COL: &str = "type";
pub(super) const _TYPE_IDX: usize = 2;

pub(super) const HIDE_FROM_REPORTS_STR: &str = "Hide from Reports";
pub(super) const _HIDE_FROM_REPORTS_COL: &str = "hide_from_reports";
pub(super) const _HIDE_FROM_REPORTS_IDX: usize = 3;

pub(super) const _CATEGORY_COL_COUNT: usize = 4;
