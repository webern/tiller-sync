//! Implements the very simple `Sheet` trait using in-memory data for testing purposes.
//!
//! Note: this is compiled even in the "production" version of this app so that we can run the whole
//! app, top-to-bottom, without using Google Sheets.

use crate::api::{Sheet, AUTO_CAT, CATEGORIES, TRANSACTIONS};
use crate::Result;
use anyhow::Context;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;

/// Type alias for sheet data: a 2D grid of strings.
type SheetData = Vec<Vec<String>>;

/// Type alias for a map of sheet name to sheet data.
type SheetDataMap = HashMap<String, SheetData>;

/// Records a call made to `TestSheet`, including the data involved.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SheetCall {
    /// A get() call was made, returning the specified data
    Get { sheet_name: String, data: SheetData },
    /// A get_formulas() call was made, returning the specified data
    GetFormulas { sheet_name: String, data: SheetData },
    /// A _put() call was made with the specified data
    _Put { sheet_name: String, data: SheetData },
}

/// An implementation of the `Sheet` trait that does not use Google sheets. It can hold any data in
/// memory and, by default, is seeded with some existing data.
///
/// This struct provides:
/// - Separate storage for values and formulas
/// - Call history tracking for test assertions
/// - Builder methods for easy test setup
pub(crate) struct TestSheet {
    /// Value data for each sheet (what `get()` returns)
    data: SheetDataMap,

    /// Formula data for each sheet (what `get_formulas()` returns).
    /// If a sheet is not present here, `get_formulas()` falls back to `data`.
    formulas: SheetDataMap,

    /// History of all calls made to this sheet. Uses RefCell for interior mutability
    /// so we can record calls even through the `&mut self` trait methods.
    call_history: RefCell<Vec<SheetCall>>,
}

// These methods are used by tests in other modules (e.g., sync tests).
// Allow dead_code until those tests are written.
#[allow(dead_code)]
impl TestSheet {
    /// Create a new empty `TestSheet`.
    pub(crate) fn new() -> Self {
        Self {
            data: HashMap::new(),
            formulas: HashMap::new(),
            call_history: RefCell::new(Vec::new()),
        }
    }

    /// Builder method: add value data for a sheet.
    pub(crate) fn with_sheet(mut self, sheet_name: &str, data: SheetData) -> Self {
        self.data.insert(sheet_name.to_string(), data);
        self
    }

    /// Builder method: add formula data for a sheet.
    /// If not set, `get_formulas()` will return the same data as `get()`.
    pub(crate) fn with_formulas(mut self, sheet_name: &str, formulas: SheetData) -> Self {
        self.formulas.insert(sheet_name.to_string(), formulas);
        self
    }

    /// Get the call history for test assertions.
    pub(crate) fn call_history(&self) -> Vec<SheetCall> {
        self.call_history.borrow().clone()
    }

    /// Clear the call history (useful between test phases).
    pub(crate) fn clear_history(&self) {
        self.call_history.borrow_mut().clear();
    }

    /// Get the current data for a sheet (useful for verifying _put results).
    pub(crate) fn get_data(&self, sheet_name: &str) -> Option<&SheetData> {
        self.data.get(sheet_name)
    }

    /// Record a call to the history.
    fn record_call(&self, call: SheetCall) {
        self.call_history.borrow_mut().push(call);
    }
}

#[async_trait::async_trait]
impl Sheet for TestSheet {
    async fn get(&mut self, sheet_name: &str) -> Result<SheetData> {
        let data = self
            .data
            .get(sheet_name)
            .with_context(|| format!("Sheet '{sheet_name}' not found"))?
            .clone();

        self.record_call(SheetCall::Get {
            sheet_name: sheet_name.to_string(),
            data: data.clone(),
        });

        Ok(data)
    }

    async fn get_formulas(&mut self, sheet_name: &str) -> Result<SheetData> {
        // If formulas are explicitly set, use them; otherwise fall back to data
        let data = if let Some(formula_data) = self.formulas.get(sheet_name) {
            formula_data.clone()
        } else {
            self.data
                .get(sheet_name)
                .with_context(|| format!("Sheet '{sheet_name}' not found"))?
                .clone()
        };

        self.record_call(SheetCall::GetFormulas {
            sheet_name: sheet_name.to_string(),
            data: data.clone(),
        });

        Ok(data)
    }

    async fn _put(&mut self, sheet_name: &str, data: &[Vec<String>]) -> crate::Result<()> {
        let data_vec = data.to_vec();

        self.record_call(SheetCall::_Put {
            sheet_name: sheet_name.to_string(),
            data: data_vec.clone(),
        });

        self.data.insert(sheet_name.to_string(), data_vec);
        Ok(())
    }
}

impl Default for TestSheet {
    /// Loads seed data from this module, including formulas for the Transactions sheet.
    fn default() -> Self {
        let (data, formulas) = default_data();
        Self {
            data,
            formulas,
            call_history: RefCell::new(Vec::new()),
        }
    }
}

/// Provides the seed data and formula data from this module.
fn default_data() -> (SheetDataMap, SheetDataMap) {
    let mut data = HashMap::new();
    let mut formulas = HashMap::new();

    // Load transactions
    let transactions = load_csv(TRANSACTION_DATA).expect("Failed to load transaction seed data");
    data.insert(TRANSACTIONS.to_string(), transactions.clone());

    // Generate formula data for transactions (formulas in "Custom Column")
    let transaction_formulas = generate_transaction_formulas(&transactions);
    formulas.insert(TRANSACTIONS.to_string(), transaction_formulas);

    // Load categories (no formulas)
    let categories = load_csv(CATEGORY_DATA).expect("Failed to load category seed data");
    data.insert(CATEGORIES.to_string(), categories);

    // Load autocat (no formulas)
    let auto_cat = load_csv(AUTO_CAT_DATA).expect("Failed to load autocat seed data");
    data.insert(AUTO_CAT.to_string(), auto_cat);

    (data, formulas)
}

/// Generates formula data for transactions, with =ABS(E{row}) formulas in "Custom Column".
fn generate_transaction_formulas(transactions: &[Vec<String>]) -> SheetData {
    if transactions.is_empty() {
        return Vec::new();
    }

    let header_row = &transactions[0];
    let custom_col_idx = header_row.iter().position(|h| h == "Custom Column");

    transactions
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            if row_idx == 0 {
                // Header row stays the same
                row.clone()
            } else if let Some(col_idx) = custom_col_idx {
                // Data row: replace Custom Column with formula
                let mut formula_row = row.clone();
                if formula_row.len() > col_idx {
                    let sheet_row_num = row_idx + 1; // 1-indexed sheet row
                    formula_row[col_idx] = format!("=ABS(E{sheet_row_num})");
                }
                formula_row
            } else {
                row.clone()
            }
        })
        .collect()
}

/// Loads data from a CSV-formatted string.
fn load_csv(csv_data: &str) -> Result<SheetData> {
    let bytes = csv_data.as_bytes(); // Get a byte slice from the String
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false) // Ensure headers are treated as part of the data
        .from_reader(Cursor::new(bytes));

    let mut rows: SheetData = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let row: Vec<String> = record.iter().map(|field| field.to_string()).collect();
        rows.push(row);
    }
    Ok(rows)
}

/// Seed transaction data.
const TRANSACTION_DATA: &str = r##",Date,Description,Category,Amount,Account,Account #,Institution,Month,Week,Transaction ID,Account ID,Check Number,Full Description,Date Added,Categorized Date,Custom Column
,10/20/2025,Whole Foods Market,Groceries,-$87.43,Credit Card 1,xxxx1234,Bank A,10/1/25,10/19/25,tx001a2b3c4d5e6f7g8h9i01,acct001a2b3c4d5e6f7g,,WHOLE FOODS MARKET,10/21/25,10/21/2025 9:15:30 AM,87.43
,10/19/2025,Starbucks #2847,Coffee Shops,-$6.75,Credit Card 1,xxxx1234,Bank A,10/1/25,10/19/25,tx001a2b3c4d5e6f7g8h9i02,acct001a2b3c4d5e6f7g,,STARBUCKS #2847,10/20/25,10/20/2025 8:45:12 AM,6.75
,10/18/2025,Shell Gas Station,Gas & Fuel,-$52.30,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i03,acct001a2b3c4d5e6f7g,,SHELL GAS STATION,10/19/25,10/19/2025 7:22:45 AM,52.30
,10/17/2025,Chipotle Mexican Grill,Restaurants,-$14.85,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i04,acct001a2b3c4d5e6f7g,,CHIPOTLE MEXICAN GRILL,10/18/25,10/18/2025 12:35:20 PM,14.85
,10/16/2025,PG&E Electric,Utilities,-$142.67,Checking 1,xxxx5678,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i05,acct002a2b3c4d5e6f7g,,PG&E ELECTRIC,10/17/25,10/17/2025 6:00:00 AM,142.67
,10/15/2025,Trader Joe's #429,Groceries,-$63.21,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i06,acct001a2b3c4d5e6f7g,,TRADER JOE'S #429,10/16/25,10/16/2025 4:18:33 PM,63.21
,10/14/2025,Peet's Coffee & Tea,Coffee Shops,-$7.25,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i07,acct001a2b3c4d5e6f7g,,PEET'S COFFEE & TEA,10/15/25,10/15/2025 9:22:18 AM,7.25
,10/13/2025,Chevron Gas,Gas & Fuel,-$48.90,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i08,acct001a2b3c4d5e6f7g,,CHEVRON GAS,10/14/25,10/14/2025 5:45:09 PM,48.90
,10/12/2025,Panera Bread,Restaurants,-$12.40,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i09,acct001a2b3c4d5e6f7g,,PANERA BREAD,10/13/25,10/13/2025 1:10:25 PM,12.40
,10/11/2025,Comcast Internet,Utilities,-$89.99,Checking 1,xxxx5678,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i10,acct002a2b3c4d5e6f7g,,COMCAST INTERNET,10/12/25,10/12/2025 6:30:00 AM,89.99
,10/10/2025,Safeway #1534,Groceries,-$95.82,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i11,acct001a2b3c4d5e6f7g,,SAFEWAY #1534,10/11/25,10/11/2025 3:42:15 PM,95.82
,10/9/2025,Blue Bottle Coffee,Coffee Shops,-$8.50,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i12,acct001a2b3c4d5e6f7g,,BLUE BOTTLE COFFEE,10/10/25,10/10/2025 10:05:44 AM,8.50
,10/8/2025,76 Gas Station,Gas & Fuel,-$55.20,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i13,acct001a2b3c4d5e6f7g,,76 GAS STATION,10/9/25,10/9/2025 6:18:52 PM,55.20
,10/7/2025,Olive Garden,Restaurants,-$42.30,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i14,acct001a2b3c4d5e6f7g,,OLIVE GARDEN,10/8/25,10/8/2025 7:25:33 PM,42.30
,10/6/2025,AT&T Wireless,Utilities,-$75.00,Checking 1,xxxx5678,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i15,acct002a2b3c4d5e6f7g,,AT&T WIRELESS,10/7/25,10/7/2025 6:00:00 AM,75.00
,10/5/2025,Costco Wholesale,Groceries,-$118.56,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i16,acct001a2b3c4d5e6f7g,,COSTCO WHOLESALE,10/6/25,10/6/2025 2:30:18 PM,118.56
,10/4/2025,Starbucks #1923,Coffee Shops,-$5.95,Credit Card 1,xxxx1234,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i17,acct001a2b3c4d5e6f7g,,STARBUCKS #1923,10/5/25,10/5/2025 8:12:05 AM,5.95
,10/3/2025,Shell Station #4521,Gas & Fuel,-$61.45,Credit Card 1,xxxx1234,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i18,acct001a2b3c4d5e6f7g,,SHELL STATION #4521,10/4/25,10/4/2025 4:55:22 PM,61.45
,10/2/2025,In-N-Out Burger,Restaurants,-$9.75,Credit Card 1,xxxx1234,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i19,acct001a2b3c4d5e6f7g,,IN-N-OUT BURGER,10/3/25,10/3/2025 6:40:11 PM,9.75
,10/1/2025,City Water District,Utilities,-$45.88,Checking 1,xxxx5678,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i20,acct002a2b3c4d5e6f7g,,CITY WATER DISTRICT,10/2/25,10/2/2025 6:00:00 AM,45.88
"##;

/// Seed category data.
const CATEGORY_DATA: &str = r##"Category,Group,Type,Hide From Reports,Jan 2024,Feb 2024,Mar 2024,Apr 2024,May 2024,Jun 2024,Jul 2024,Aug 2024,Sep 2024,Oct 2024,Nov 2024,Dec 2024
Groceries,Food,Expense,,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00
Coffee Shops,Food,Expense,,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00
Gas & Fuel,Auto,Expense,,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00
Restaurants,Food,Expense,,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00
Utilities,Home,Expense,,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00,$0.00
"##;

/// Seed AutoCat data.
const AUTO_CAT_DATA: &str = r##"Category,Description Contains,Account Contains,Institution Contains,Amount Min,Amount Max,Amount Equals,Description Equals,Description,Full Description Contains,Amount Contains
Groceries,Whole Foods,,,,,,,,,
Coffee Shops,Starbucks,,,,,,,,,
Gas & Fuel,Shell,,,,,,,,,
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builder_and_call_history() {
        // Build a TestSheet with custom data
        let mut sheet = TestSheet::new()
            .with_sheet(
                "TestTab",
                vec![
                    vec!["Header1".to_string(), "Header2".to_string()],
                    vec!["Value1".to_string(), "Value2".to_string()],
                ],
            )
            .with_formulas(
                "TestTab",
                vec![
                    vec!["Header1".to_string(), "Header2".to_string()],
                    vec!["Value1".to_string(), "=A2".to_string()],
                ],
            );

        // Initially, call history should be empty
        assert!(sheet.call_history().is_empty());

        // Call get and verify it's recorded
        let data = sheet.get("TestTab").await.unwrap();
        assert_eq!(data[1][0], "Value1");

        let history = sheet.call_history();
        assert_eq!(history.len(), 1);
        assert!(
            matches!(&history[0], SheetCall::Get { sheet_name, .. } if sheet_name == "TestTab")
        );

        // Call get_formulas and verify we get the formula data
        let formulas = sheet.get_formulas("TestTab").await.unwrap();
        assert_eq!(formulas[1][1], "=A2");

        let history = sheet.call_history();
        assert_eq!(history.len(), 2);
        assert!(
            matches!(&history[1], SheetCall::GetFormulas { sheet_name, .. } if sheet_name == "TestTab")
        );

        // Call _put and verify it updates data and is recorded
        let new_data = vec![vec!["NewHeader".to_string()], vec!["NewValue".to_string()]];
        sheet._put("TestTab", &new_data).await.unwrap();

        // Verify _put updated the stored data
        let stored = sheet.get_data("TestTab").unwrap();
        assert_eq!(stored[0][0], "NewHeader");

        let history = sheet.call_history();
        assert_eq!(history.len(), 3);
        assert!(
            matches!(&history[2], SheetCall::_Put { sheet_name, .. } if sheet_name == "TestTab")
        );

        // Test clear_history
        sheet.clear_history();
        assert!(sheet.call_history().is_empty());
    }

    #[tokio::test]
    async fn test_get_formulas_falls_back_to_data() {
        // When formulas are not set, get_formulas should return the same as get
        let mut sheet = TestSheet::new().with_sheet(
            "NoFormulas",
            vec![
                vec!["A".to_string(), "B".to_string()],
                vec!["1".to_string(), "2".to_string()],
            ],
        );

        let data = sheet.get("NoFormulas").await.unwrap();
        let formulas = sheet.get_formulas("NoFormulas").await.unwrap();

        assert_eq!(data, formulas);
    }
}
