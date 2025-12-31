use crate::error::Res;
use crate::model::{Mapping, RowCol};
use anyhow::{bail, Context};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Debug;

/// Represents the row data from a sheet, including the header mapping.
#[derive(Default, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(bound(deserialize = "I: DeserializeOwned"))]
pub struct Items<I>
where
    I: Default + Debug + Clone + Eq + PartialEq + Serialize + DeserializeOwned + Item,
{
    mapping: Mapping,
    data: Vec<I>,
    /// Maps (row_index, column_index) -> formula for cells that contain formulas.
    /// Stored exactly as returned by the Google Sheets API.
    formulas: BTreeMap<RowCol, String>,
}

pub trait Item {
    /// Given the `header` name and the `value`, set the appropriate struct field.
    fn set_with_header<S1, S2>(&mut self, header: S1, value: S2) -> Res<()>
    where
        S1: AsRef<str>,
        S2: Into<String>;

    /// Given the `header` name, retrieve the appropriate struct field value.
    fn get_by_header(&self, header: &str) -> String;

    /// Given the order of the `headers`, convert the struct field values to a `Vec<String>` where
    /// the values appear in the right order according to the `headers` order.
    fn to_row(&self, headers: &[String]) -> Vec<String> {
        headers.iter().map(|h| self.get_by_header(h)).collect()
    }

    /// Set the field named `original_order` which is the row index in which the data row appeared
    /// in the spreadsheet during download.
    fn set_original_order(&mut self, original_order: u64);

    /// Get the field named `original_order` which is the row index in which the data row appeared
    /// in the spreadsheet during download.
    fn get_original_order(&self) -> Option<u64>;
}

fn parse_row<S1, S2, Iter, T>(headers: &[S1], values: Iter, original_order: u64) -> Res<T>
where
    S1: AsRef<str>,
    S2: Into<String>,
    Iter: IntoIterator<Item = S2>,
    T: Default + Debug + Clone + Eq + PartialEq + Serialize + DeserializeOwned + Item,
{
    let mut transaction = T::default();
    for (ix, value) in values.into_iter().map(|s| s.into()).enumerate() {
        let header = headers
            .get(ix)
            .with_context(|| format!("No header found for column index {ix}"))?
            .as_ref();
        transaction.set_with_header(header, value)?;
    }
    transaction.set_original_order(original_order);
    Ok(transaction)
}

impl<I> Items<I>
where
    I: Default + Debug + Clone + Eq + PartialEq + Serialize + DeserializeOwned + Item,
{
    /// Given the downloaded data from a sheet, parse the headers, data and formulas into a `Items`
    /// structure.
    ///
    /// These generics are confusing, but think of it like this: both `sheet_data` and
    /// `formula_data` are iterators into something that looks like `Vec<Vec<String>>`, i.e. rows.
    pub(crate) fn parse<S, R, I1, I2>(sheet_data: I1, formula_data: I2) -> Res<Self>
    where
        S: Into<String>,
        R: IntoIterator<Item = S>,
        I1: IntoIterator<Item = R>,
        I2: IntoIterator<Item = R>,
    {
        let mut rows = sheet_data.into_iter();
        let mapping = match rows.next() {
            Some(header_row) => Mapping::new(header_row.into_iter())?,
            None => bail!("An empty data set cannot be parsed into an Items object"),
        };

        let len = mapping.len();

        // Convert formula data to Vec<Vec<String>> for comparison
        let formula_rows: Vec<Vec<String>> = formula_data
            .into_iter()
            .map(|row| row.into_iter().map(|s| s.into()).collect())
            .collect();

        // Detect formulas by comparing values vs formulas
        let mut formulas = BTreeMap::new();
        let mut items = Vec::new();

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
                        // If it starts with '=' and does not match the value, then it's a formula
                        if formula.starts_with('=') && formula != value {
                            formulas.insert(RowCol::new(row_ix, col_ix), formula.clone());
                        }
                    }
                }
            }

            let txn: I = parse_row(mapping.headers(), values, row_ix as u64)?;
            items.push(txn);
        }
        Ok(Self {
            mapping,
            data: items,
            formulas,
        })
    }

    /// Creates a new Items object when the data has been fully parsed and the header Mapping exists
    pub(crate) fn new(
        data: Vec<I>,
        formulas: BTreeMap<RowCol, String>,
        mapping: Mapping,
    ) -> Res<Self> {
        if mapping.headers().is_empty() {
            bail!("We cannot proceed without headers.")
        }

        Ok(Self {
            mapping,
            data,
            formulas,
        })
    }

    /// Converts the transactions to rows suitable for writing to a Google Sheet.
    /// Returns (headers, data_rows) where headers is the column names and data_rows
    /// contains the transaction data in the same column order.
    pub(crate) fn to_rows(&self) -> Res<Vec<Vec<String>>> {
        // Use mapping headers if available, otherwise use default headers
        let headers: Vec<String> = if self.mapping().headers().is_empty() {
            bail!("The headers are missing")
        } else {
            self.mapping()
                .headers()
                .iter()
                .map(|s| s.as_ref().to_string())
                .collect()
        };

        let mut rows = vec![headers.clone()];

        rows.extend(self.data().iter().map(|txn| txn.to_row(&headers)));

        Ok(rows)
    }

    pub fn data(&self) -> &Vec<I> {
        &self.data
    }

    pub fn mapping(&self) -> &Mapping {
        &self.mapping
    }

    pub fn formulas(&self) -> &BTreeMap<RowCol, String> {
        &self.formulas
    }
}
