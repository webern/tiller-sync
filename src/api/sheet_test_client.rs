//! Implements the very simple `Sheet` trait using in-memory data for testing purposes.
//!
//! Note: this is compiled even in the "production" version of this app so that we can run the whole
//! app, top-to-bottom, without using Google Sheets.

use crate::api::{Sheet, AUTO_CAT, CATEGORIES, TRANSACTIONS};
use crate::Result;
use anyhow::Context;
use std::collections::HashMap;
use std::io::Cursor;

/// An implementation of the `Sheet` trait that does not use Google sheets. It can hold any data in
/// memory and, by default, is seeded with some existing data.
pub(crate) struct TestSheet {
    pub(crate) data: HashMap<String, Vec<Vec<String>>>,
}

impl TestSheet {
    /// Create a new `TestSheet` using `data`. The map key is sheet name and the map value is the
    /// rows of the sheet.
    pub(crate) fn new(data: HashMap<String, Vec<Vec<String>>>) -> Self {
        Self { data }
    }
}

#[async_trait::async_trait]
impl Sheet for TestSheet {
    async fn get(&mut self, sheet_name: &str) -> Result<Vec<Vec<String>>> {
        self.data
            .get(sheet_name)
            .with_context(|| format!("Sheet '{sheet_name}' not found"))
            .cloned()
    }

    async fn _put(&mut self, sheet_name: &str, data: &[Vec<String>]) -> crate::Result<()> {
        self.data.insert(sheet_name.to_string(), data.to_vec());
        Ok(())
    }
}

impl Default for TestSheet {
    /// Loads seed data from this module.
    fn default() -> Self {
        Self::new(default_data())
    }
}

/// Provides the seed data from this module.
fn default_data() -> HashMap<String, Vec<Vec<String>>> {
    let mut map = HashMap::new();
    let transactions = load_csv(TRANSACTION_DATA).unwrap();
    map.insert(TRANSACTIONS.to_string(), transactions);
    let categories = load_csv(CATEGORY_DATA).unwrap();
    map.insert(CATEGORIES.to_string(), categories);
    let auto_cat = load_csv(AUTO_CAT_DATA).unwrap();
    map.insert(AUTO_CAT.to_string(), auto_cat);
    map
}

/// Loads data from a CSV-formatted string.
fn load_csv(csv_data: &str) -> Result<Vec<Vec<String>>> {
    let bytes = csv_data.as_bytes(); // Get a byte slice from the String
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false) // Ensure headers are treated as part of the data
        .from_reader(Cursor::new(bytes));

    let mut rows: Vec<Vec<String>> = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let row: Vec<String> = record.iter().map(|field| field.to_string()).collect();
        rows.push(row);
    }
    Ok(rows)
}

/// Seed transaction data.
const TRANSACTION_DATA: &str = r##",Date,Description,Category,Amount,Account,Account #,Institution,Month,Week,Transaction ID,Account ID,Check Number,Full Description,Date Added,Categorized Date
,10/20/2025,Whole Foods Market,Groceries,-$87.43,Credit Card 1,xxxx1234,Bank A,10/1/25,10/19/25,tx001a2b3c4d5e6f7g8h9i01,acct001a2b3c4d5e6f7g,,WHOLE FOODS MARKET,10/21/25,10/21/2025 9:15:30 AM
,10/19/2025,Starbucks #2847,Coffee Shops,-$6.75,Credit Card 1,xxxx1234,Bank A,10/1/25,10/19/25,tx001a2b3c4d5e6f7g8h9i02,acct001a2b3c4d5e6f7g,,STARBUCKS #2847,10/20/25,10/20/2025 8:45:12 AM
,10/18/2025,Shell Gas Station,Gas & Fuel,-$52.30,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i03,acct001a2b3c4d5e6f7g,,SHELL GAS STATION,10/19/25,10/19/2025 7:22:45 AM
,10/17/2025,Chipotle Mexican Grill,Restaurants,-$14.85,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i04,acct001a2b3c4d5e6f7g,,CHIPOTLE MEXICAN GRILL,10/18/25,10/18/2025 12:35:20 PM
,10/16/2025,PG&E Electric,Utilities,-$142.67,Checking 1,xxxx5678,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i05,acct002a2b3c4d5e6f7g,,PG&E ELECTRIC,10/17/25,10/17/2025 6:00:00 AM
,10/15/2025,Trader Joe's #429,Groceries,-$63.21,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i06,acct001a2b3c4d5e6f7g,,TRADER JOE'S #429,10/16/25,10/16/2025 4:18:33 PM
,10/14/2025,Peet's Coffee & Tea,Coffee Shops,-$7.25,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i07,acct001a2b3c4d5e6f7g,,PEET'S COFFEE & TEA,10/15/25,10/15/2025 9:22:18 AM
,10/13/2025,Chevron Gas,Gas & Fuel,-$48.90,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i08,acct001a2b3c4d5e6f7g,,CHEVRON GAS,10/14/25,10/14/2025 5:45:09 PM
,10/12/2025,Panera Bread,Restaurants,-$12.40,Credit Card 1,xxxx1234,Bank A,10/1/25,10/12/25,tx001a2b3c4d5e6f7g8h9i09,acct001a2b3c4d5e6f7g,,PANERA BREAD,10/13/25,10/13/2025 1:10:25 PM
,10/11/2025,Comcast Internet,Utilities,-$89.99,Checking 1,xxxx5678,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i10,acct002a2b3c4d5e6f7g,,COMCAST INTERNET,10/12/25,10/12/2025 6:30:00 AM
,10/10/2025,Safeway #1534,Groceries,-$95.82,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i11,acct001a2b3c4d5e6f7g,,SAFEWAY #1534,10/11/25,10/11/2025 3:42:15 PM
,10/9/2025,Blue Bottle Coffee,Coffee Shops,-$8.50,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i12,acct001a2b3c4d5e6f7g,,BLUE BOTTLE COFFEE,10/10/25,10/10/2025 10:05:44 AM
,10/8/2025,76 Gas Station,Gas & Fuel,-$55.20,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i13,acct001a2b3c4d5e6f7g,,76 GAS STATION,10/9/25,10/9/2025 6:18:52 PM
,10/7/2025,Olive Garden,Restaurants,-$42.30,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i14,acct001a2b3c4d5e6f7g,,OLIVE GARDEN,10/8/25,10/8/2025 7:25:33 PM
,10/6/2025,AT&T Wireless,Utilities,-$75.00,Checking 1,xxxx5678,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i15,acct002a2b3c4d5e6f7g,,AT&T WIRELESS,10/7/25,10/7/2025 6:00:00 AM
,10/5/2025,Costco Wholesale,Groceries,-$118.56,Credit Card 1,xxxx1234,Bank A,10/1/25,10/5/25,tx001a2b3c4d5e6f7g8h9i16,acct001a2b3c4d5e6f7g,,COSTCO WHOLESALE,10/6/25,10/6/2025 2:30:18 PM
,10/4/2025,Starbucks #1923,Coffee Shops,-$5.95,Credit Card 1,xxxx1234,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i17,acct001a2b3c4d5e6f7g,,STARBUCKS #1923,10/5/25,10/5/2025 8:12:05 AM
,10/3/2025,Shell Station #4521,Gas & Fuel,-$61.45,Credit Card 1,xxxx1234,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i18,acct001a2b3c4d5e6f7g,,SHELL STATION #4521,10/4/25,10/4/2025 4:55:22 PM
,10/2/2025,In-N-Out Burger,Restaurants,-$9.75,Credit Card 1,xxxx1234,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i19,acct001a2b3c4d5e6f7g,,IN-N-OUT BURGER,10/3/25,10/3/2025 6:40:11 PM
,10/1/2025,City Water District,Utilities,-$45.88,Checking 1,xxxx5678,Bank A,10/1/25,9/28/25,tx001a2b3c4d5e6f7g8h9i20,acct002a2b3c4d5e6f7g,,CITY WATER DISTRICT,10/2/25,10/2/2025 6:00:00 AM
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
