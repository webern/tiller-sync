//! Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.

use crate::api::{Sheet, Tiller, AUTO_CAT, CATEGORIES, TRANSACTIONS};
use crate::model::{Amount, AutoCat, Category, TillerData, Transaction};
use crate::Result;
use anyhow::Context;
use std::collections::HashMap;
use std::str::FromStr;

/// Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.
pub(super) struct TillerImpl {
    sheet: Box<dyn Sheet + Send>,
}

impl TillerImpl {
    /// Create a new `TillerImpl` object that will use a dynamically-dispatched `sheet` to get and
    /// send its data.
    pub(super) async fn new(sheet: Box<dyn Sheet + Send>) -> Result<Self> {
        Ok(Self { sheet })
    }
}

#[async_trait::async_trait]
impl Tiller for TillerImpl {
    async fn get_data(&mut self) -> Result<TillerData> {
        // Fetch data from all three tabs
        let transactions = fetch_transactions(self.sheet.as_mut()).await?;
        let categories = fetch_categories(self.sheet.as_mut()).await?;
        let auto_cats = fetch_auto_cats(self.sheet.as_mut()).await?;

        Ok(TillerData {
            transactions,
            categories,
            auto_cats,
        })
    }
}

/// Fetches transaction data from the Transactions tab
async fn fetch_transactions(client: &mut (dyn Sheet + Send)) -> Result<Vec<Transaction>> {
    let values = client.get(TRANSACTIONS).await?;
    if values.is_empty() {
        return Ok(Vec::new());
    }

    // First row is the header
    let header = &values[0];
    let column_map = build_column_map(header);

    // Data rows start from index 1
    let data_rows = &values[1..];

    let mut transactions = Vec::new();
    for row in data_rows {
        // Skip empty rows
        if row.is_empty() {
            continue;
        }

        let transaction = Transaction {
            transaction_id: get_cell_by_name(row, &column_map, "Transaction ID"),
            date: get_cell_by_name(row, &column_map, "Date"),
            description: get_cell_by_name(row, &column_map, "Description"),
            amount: parse_amount(&get_cell_by_name(row, &column_map, "Amount"))?,
            account: get_cell_by_name(row, &column_map, "Account"),
            account_number: get_cell_by_name(row, &column_map, "Account #"),
            institution: get_cell_by_name(row, &column_map, "Institution"),
            month: get_cell_by_name(row, &column_map, "Month"),
            week: get_cell_by_name(row, &column_map, "Week"),
            full_description: get_cell_by_name(row, &column_map, "Full Description"),
            account_id: get_cell_by_name(row, &column_map, "Account ID"),
            check_number: get_cell_by_name(row, &column_map, "Check Number"),
            date_added: get_cell_by_name(row, &column_map, "Date Added"),
            merchant_name: get_cell_by_name(row, &column_map, "Merchant Name"),
            category_hint: get_cell_by_name(row, &column_map, "Category Hint"),
            category: get_cell_by_name(row, &column_map, "Category"),
            note: get_cell_by_name(row, &column_map, "Note"),
            tags: get_cell_by_name(row, &column_map, "Tags"),
        };

        transactions.push(transaction);
    }

    Ok(transactions)
}

/// Fetches category data from the Categories tab
async fn fetch_categories(client: &mut (dyn Sheet + Send)) -> Result<Vec<Category>> {
    let values = client.get(CATEGORIES).await?;
    if values.is_empty() {
        return Ok(Vec::new());
    }

    // First row is the header
    let header = &values[0];
    let column_map = build_column_map(header);

    // Data rows start from index 1
    let data_rows = &values[1..];

    let mut categories = Vec::new();
    for row in data_rows {
        if row.is_empty() {
            continue;
        }

        let category = Category {
            category: get_cell_by_name(row, &column_map, "Category"),
            group: get_cell_by_name(row, &column_map, "Group"),
            _type: get_cell_by_name(row, &column_map, "Type"),
            hide_from_reports: get_cell_by_name(row, &column_map, "Hide from Reports"),
        };

        categories.push(category);
    }

    Ok(categories)
}

/// Fetches AutoCat data from the AutoCat tab
async fn fetch_auto_cats(client: &mut (dyn Sheet + Send)) -> Result<Vec<AutoCat>> {
    let values = client.get(AUTO_CAT).await?;
    if values.is_empty() {
        return Ok(Vec::new());
    }

    // First row is the header
    let header = &values[0];
    let column_map = build_column_map(header);

    // Data rows start from index 1
    let data_rows = &values[1..];

    let mut auto_cats = Vec::new();
    for row in data_rows {
        if row.is_empty() {
            continue;
        }

        let auto_cat = AutoCat {
            category: get_cell_by_name(row, &column_map, "Category"),
            description_contains: get_optional_cell_by_name(
                row,
                &column_map,
                "Description Contains",
            ),
            account_contains: get_optional_cell_by_name(row, &column_map, "Account Contains"),
            institution_contains: get_optional_cell_by_name(
                row,
                &column_map,
                "Institution Contains",
            ),
            amount_min: parse_optional_amount(&get_cell_by_name(row, &column_map, "Amount Min"))?,
            amount_max: parse_optional_amount(&get_cell_by_name(row, &column_map, "Amount Max"))?,
            amount_equals: parse_optional_amount(&get_cell_by_name(
                row,
                &column_map,
                "Amount Equals",
            ))?,
            description_equals: get_optional_cell_by_name(row, &column_map, "Description Equals"),
            description_full: get_optional_cell_by_name(row, &column_map, "Description Full"),
            full_description_contains: get_optional_cell_by_name(
                row,
                &column_map,
                "Full Description Contains",
            ),
            amount_contains: get_optional_cell_by_name(row, &column_map, "Amount Contains"),
        };

        auto_cats.push(auto_cat);
    }

    Ok(auto_cats)
}

/// Builds a mapping from column names (lowercase) to their indices
fn build_column_map(header: &[String]) -> HashMap<String, usize> {
    header
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_lowercase(), i))
        .collect()
}

/// Gets a cell value by column name (case-insensitive), returning an empty string if not found
fn get_cell_by_name(row: &[String], column_map: &HashMap<String, usize>, name: &str) -> String {
    column_map
        .get(&name.to_lowercase())
        .and_then(|&index| row.get(index))
        .cloned()
        .unwrap_or_default()
}

/// Gets an optional cell value by column name, returning None if empty or not found
fn get_optional_cell_by_name(
    row: &[String],
    column_map: &HashMap<String, usize>,
    name: &str,
) -> Option<String> {
    let value = get_cell_by_name(row, column_map, name);
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Parses a string into an Amount, handling empty strings and dollar signs
fn parse_amount(s: &str) -> Result<Amount> {
    Amount::from_str(s).context(format!("Failed to parse amount value: {s}"))
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
