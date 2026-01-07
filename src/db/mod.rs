//! This module is responsible for reading, writing and managing the SQLite database. The internal
//! details of SQLite interaction are hidden while broader functions are exposed.

mod migrations;

use crate::api::{AUTO_CAT, CATEGORIES, TRANSACTIONS};
use crate::error::Res;
use crate::model::{Amount, AutoCat, Category, Mapping, TillerData, Transaction};
use anyhow::{bail, Context};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

/// The target schema version for the database. This equals the highest migration number available.
/// When `migration_05_up.sql` is the highest numbered migration, this should be `5`.
pub(crate) const CURRENT_VERSION: i32 = 1;

/// Represents a row in the database in a table for which the primary key is not known in
/// `TillerData`. Namely, rows from the `categories` and `autocats` tables.
#[derive(Default, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct _Row<T> {
    /// The primary key identifier for this row in the database.
    pub id: u64,
    /// The data row.
    pub row: T,
}

/// A local SQLite datastore for tiller transaction data. This object abstracts away any knowledge
/// of the underlying datastore details and presents high level functions for interacting with the
/// data that is held in the datastore.
///
/// Note for AI Agents:
/// - NEVER: pub(crate) sql_connection() -> Pool : this is low level and should be private
/// - YES: pub(crate) transactions(&self) -> Transactions : correct level of abstraction
///
#[derive(Debug, Clone)]
pub(crate) struct Db {
    pool: SqlitePool,
}

impl Db {
    /// - Validates that there is a SQLite file at `path`
    /// - Creates a SQLite client
    /// - Updates the database schema with migrations if it is out-of-date
    /// - Returns a constructed `Datastore` object for further operations
    pub(crate) async fn load(path: impl AsRef<Path>) -> Res<Self> {
        let path = path.as_ref();
        if !path.exists() {
            bail!("SQLite database not found at {}", path.display());
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .context("Failed to parse SQLite connection string")?
            .create_if_missing(false)
            // Enable foreign key constraints by default
            .pragma("foreign_keys", "ON");

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .context("Failed to connect to SQLite database")?;

        let db = Self { pool };
        db.migrate().await?;

        Ok(db)
    }

    /// - Validates that no file currently exists at `path`
    /// - Creates a new SQLite file at `path`
    /// - Initializes the database schema
    /// - Returns a constructed `Datastore` object for further operations
    pub(crate) async fn init(path: impl AsRef<Path>) -> Res<Self> {
        let path = path.as_ref();
        if path.exists() {
            bail!("SQLite database already exists at {}", path.display());
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .context("Failed to parse SQLite connection string")?
            .create_if_missing(true)
            // Enable foreign key constraints by default
            .pragma("foreign_keys", "ON");

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .context("Failed to create SQLite database")?;

        let db = Self { pool };
        db.bootstrap().await?;
        db.migrate().await?;

        Ok(db)
    }

    /// Returns the number of rows in the transactions table.
    pub(crate) async fn count_transactions(&self) -> Res<u64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0 as u64)
    }

    /// Saves TillerData into the database.
    /// - Transactions: upsert (insert new, update existing, delete removed)
    /// - Categories: delete all, then insert all
    /// - AutoCat: delete all, then insert all
    ///
    /// Note: Foreign key constraints are temporarily disabled during this operation
    /// to allow the delete-all-then-insert pattern for categories and autocat.
    pub(crate) async fn save_tiller_data(&self, data: &TillerData) -> Res<()> {
        // Disable foreign key constraints for bulk sync operation
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&self.pool)
            .await?;

        // Use a closure to ensure FK constraints are re-enabled even on error
        let result = self.save_tiller_data_inner(data).await;

        // Re-enable foreign key constraints
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;

        result
    }

    /// Inner implementation of save_tiller_data, called with FK constraints disabled.
    async fn save_tiller_data_inner(&self, data: &TillerData) -> Res<()> {
        use sqlx::Row;

        // Get existing transaction IDs for upsert logic
        let existing_ids: Vec<String> = sqlx::query("SELECT transaction_id FROM transactions")
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| row.get("transaction_id"))
            .collect();

        let incoming_ids: std::collections::HashSet<&str> = data
            .transactions
            .data()
            .iter()
            .map(|t| t.transaction_id.as_str())
            .collect();

        // Delete transactions that no longer exist
        for id in &existing_ids {
            if !incoming_ids.contains(id.as_str()) {
                sqlx::query("DELETE FROM transactions WHERE transaction_id = ?")
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
        }

        // Upsert transactions
        let existing_set: std::collections::HashSet<&str> =
            existing_ids.iter().map(|s| s.as_str()).collect();
        for transaction in data.transactions.data() {
            if existing_set.contains(transaction.transaction_id.as_str()) {
                self.update_transaction(transaction).await?;
            } else {
                self._insert_transaction(transaction).await?;
            }
        }

        // Categories: delete all, then insert all
        sqlx::query("DELETE FROM categories")
            .execute(&self.pool)
            .await?;
        for category in data.categories.data() {
            self._insert_category(category).await?;
        }

        // AutoCat: delete all, then insert all
        sqlx::query("DELETE FROM autocat")
            .execute(&self.pool)
            .await?;
        for autocat in data.auto_cats.data() {
            self._insert_autocat(autocat).await?;
        }

        // Save formulas from all sheets
        self.save_formulas(data).await?;

        // Save sheet metadata (header mappings) for all sheets
        self.save_sheet_metadata(data).await?;

        Ok(())
    }

    /// Saves formulas from TillerData to the formulas table.
    /// Clears existing formulas and inserts all formulas from the three sheets.
    async fn save_formulas(&self, data: &TillerData) -> Res<()> {
        // Clear all existing formulas
        sqlx::query("DELETE FROM formulas")
            .execute(&self.pool)
            .await?;

        // Save transaction formulas
        for (row_col, formula) in data.transactions.formulas() {
            sqlx::query("INSERT INTO formulas (sheet, row, col, formula) VALUES (?, ?, ?, ?)")
                .bind(TRANSACTIONS)
                .bind(row_col.0 as i64)
                .bind(row_col.1 as i64)
                .bind(formula)
                .execute(&self.pool)
                .await
                .context("Failed to insert transaction formula")?;
        }

        // Save category formulas
        for (row_col, formula) in data.categories.formulas() {
            sqlx::query("INSERT INTO formulas (sheet, row, col, formula) VALUES (?, ?, ?, ?)")
                .bind(CATEGORIES)
                .bind(row_col.0 as i64)
                .bind(row_col.1 as i64)
                .bind(formula)
                .execute(&self.pool)
                .await
                .context("Failed to insert category formula")?;
        }

        // Save autocat formulas
        for (row_col, formula) in data.auto_cats.formulas() {
            sqlx::query("INSERT INTO formulas (sheet, row, col, formula) VALUES (?, ?, ?, ?)")
                .bind(AUTO_CAT)
                .bind(row_col.0 as i64)
                .bind(row_col.1 as i64)
                .bind(formula)
                .execute(&self.pool)
                .await
                .context("Failed to insert autocat formula")?;
        }

        Ok(())
    }

    /// Saves sheet metadata (header mapping) for all three sheets.
    /// Clears existing metadata and inserts all mappings.
    async fn save_sheet_metadata(&self, data: &TillerData) -> Res<()> {
        // Clear all existing metadata
        sqlx::query("DELETE FROM sheet_metadata")
            .execute(&self.pool)
            .await?;

        // Helper to save mapping for a sheet
        async fn save_mapping(pool: &SqlitePool, sheet: &str, mapping: &Mapping) -> Res<()> {
            for (order, (header, column)) in mapping
                .headers()
                .iter()
                .zip(mapping.columns().iter())
                .enumerate()
            {
                let header_str: &str = header.as_ref();
                let column_str: &str = column.as_ref();
                sqlx::query(
                    r#"INSERT INTO sheet_metadata (sheet, column_name, header_name, "order")
                       VALUES (?, ?, ?, ?)"#,
                )
                .bind(sheet)
                .bind(column_str)
                .bind(header_str)
                .bind(order as i64)
                .execute(pool)
                .await
                .context("Failed to insert sheet metadata")?;
            }
            Ok(())
        }

        // Save metadata for each sheet
        save_mapping(&self.pool, TRANSACTIONS, data.transactions.mapping()).await?;
        save_mapping(&self.pool, CATEGORIES, data.categories.mapping()).await?;
        save_mapping(&self.pool, AUTO_CAT, data.auto_cats.mapping()).await?;

        Ok(())
    }

    /// Loads sheet metadata (header mapping) for a specific sheet.
    /// Returns None if no metadata exists for the sheet.
    async fn load_sheet_metadata(&self, sheet: &str) -> Res<Option<Mapping>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"SELECT header_name FROM sheet_metadata
               WHERE sheet = ?
               ORDER BY "order" ASC"#,
        )
        .bind(sheet)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let headers: Vec<String> = rows.into_iter().map(|(h,)| h).collect();
        let mapping =
            Mapping::new(headers).context("Failed to create Mapping from sheet_metadata")?;
        Ok(Some(mapping))
    }

    /// Retrieves all data from the database as TillerData.
    pub(crate) async fn get_tiller_data(&self) -> Res<TillerData> {
        use crate::model::{AutoCats, Categories, Transactions};
        use sqlx::Row;

        // Query all transactions
        let rows = sqlx::query(
            r#"SELECT
                transaction_id, date, description, amount, account, account_number,
                institution, month, week, full_description, account_id, check_number,
                date_added, merchant_name, category_hint, category, note, tags,
                categorized_date, statement, metadata, other_fields, original_order
            FROM transactions ORDER BY original_order ASC NULLS LAST, transaction_id ASC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut transactions_data = Vec::new();
        for r in rows {
            let other_fields_json: Option<String> = r.get("other_fields");
            let other_fields: BTreeMap<String, String> = match other_fields_json {
                Some(json) => serde_json::from_str(&json)?,
                None => BTreeMap::new(),
            };

            let amount_val: f64 = r
                .try_get::<f64, _>("amount")
                .or_else(|_| r.try_get::<i64, _>("amount").map(|i| i as f64))
                .unwrap_or(0.0);

            transactions_data.push(Transaction {
                transaction_id: r.get("transaction_id"),
                date: r.get("date"),
                description: r.get("description"),
                amount: Amount::new(Decimal::from_f64(amount_val).unwrap_or_default()),
                account: r.get("account"),
                account_number: r.get("account_number"),
                institution: r.get("institution"),
                month: r.get::<Option<String>, _>("month").unwrap_or_default(),
                week: r.get::<Option<String>, _>("week").unwrap_or_default(),
                full_description: r
                    .get::<Option<String>, _>("full_description")
                    .unwrap_or_default(),
                account_id: r.get("account_id"),
                check_number: r
                    .get::<Option<String>, _>("check_number")
                    .unwrap_or_default(),
                date_added: r.get::<Option<String>, _>("date_added").unwrap_or_default(),
                merchant_name: r
                    .get::<Option<String>, _>("merchant_name")
                    .unwrap_or_default(),
                category_hint: r
                    .get::<Option<String>, _>("category_hint")
                    .unwrap_or_default(),
                category: r.get::<Option<String>, _>("category").unwrap_or_default(),
                note: r.get::<Option<String>, _>("note").unwrap_or_default(),
                tags: r.get::<Option<String>, _>("tags").unwrap_or_default(),
                categorized_date: r
                    .get::<Option<String>, _>("categorized_date")
                    .unwrap_or_default(),
                statement: r.get::<Option<String>, _>("statement").unwrap_or_default(),
                metadata: r.get::<Option<String>, _>("metadata").unwrap_or_default(),
                other_fields,
                original_order: r.get::<Option<u64>, _>("original_order"),
                ..Default::default()
            });
        }

        // Query all categories
        let rows = sqlx::query(
            "SELECT category, category_group, type, hide_from_reports, other_fields, original_order FROM categories ORDER BY original_order ASC NULLS LAST, category ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut categories_data = Vec::new();
        for r in rows {
            let other_fields_json: Option<String> = r.get("other_fields");
            let other_fields: BTreeMap<String, String> = match other_fields_json {
                Some(json) => serde_json::from_str(&json)?,
                None => BTreeMap::new(),
            };

            categories_data.push(Category {
                category: r.get("category"),
                category_group: r
                    .get::<Option<String>, _>("category_group")
                    .unwrap_or_default(),
                r#type: r.get::<Option<String>, _>("type").unwrap_or_default(),
                hide_from_reports: r
                    .get::<Option<String>, _>("hide_from_reports")
                    .unwrap_or_default(),
                other_fields,
                original_order: r.get::<Option<u64>, _>("original_order"),
            });
        }

        // Query all autocat rules
        let rows = sqlx::query(
            r#"SELECT id, category, description, description_contains, account_contains,
                institution_contains, amount_min, amount_max, amount_equals,
                description_equals, description_full, full_description_contains,
                amount_contains, other_fields, original_order
            FROM autocat ORDER BY original_order ASC NULLS LAST, id ASC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut autocat_data = Vec::new();
        for r in rows {
            let other_fields_json: Option<String> = r.get("other_fields");
            let other_fields: BTreeMap<String, String> = match other_fields_json {
                Some(json) => serde_json::from_str(&json)?,
                None => BTreeMap::new(),
            };

            let amount_min: Option<String> = r.get("amount_min");
            let amount_max: Option<String> = r.get("amount_max");
            let amount_equals: Option<String> = r.get("amount_equals");

            autocat_data.push(AutoCat {
                category: r.get::<Option<String>, _>("category").unwrap_or_default(),
                description: r
                    .get::<Option<String>, _>("description")
                    .unwrap_or_default(),
                description_contains: r
                    .get::<Option<String>, _>("description_contains")
                    .unwrap_or_default(),
                account_contains: r
                    .get::<Option<String>, _>("account_contains")
                    .unwrap_or_default(),
                institution_contains: r
                    .get::<Option<String>, _>("institution_contains")
                    .unwrap_or_default(),
                amount_min: amount_min.and_then(|v| v.parse().ok()),
                amount_max: amount_max.and_then(|v| v.parse().ok()),
                amount_equals: amount_equals.and_then(|v| v.parse().ok()),
                description_equals: r
                    .get::<Option<String>, _>("description_equals")
                    .unwrap_or_default(),
                description_full: r
                    .get::<Option<String>, _>("description_full")
                    .unwrap_or_default(),
                full_description_contains: r
                    .get::<Option<String>, _>("full_description_contains")
                    .unwrap_or_default(),
                amount_contains: r
                    .get::<Option<String>, _>("amount_contains")
                    .unwrap_or_default(),
                other_fields,
                original_order: r.get::<Option<u64>, _>("original_order"),
            });
        }

        // Query formulas for all sheets
        let formula_rows: Vec<(String, i64, i64, String)> = sqlx::query_as(
            "SELECT sheet, row, col, formula FROM formulas ORDER BY sheet, row, col",
        )
        .fetch_all(&self.pool)
        .await?;

        // Build formula maps for each sheet
        use crate::model::RowCol;
        use std::collections::BTreeMap;

        let mut txn_formulas: BTreeMap<RowCol, String> = BTreeMap::new();
        let mut cat_formulas: BTreeMap<RowCol, String> = BTreeMap::new();
        let mut autocat_formulas: BTreeMap<RowCol, String> = BTreeMap::new();

        for (sheet, row, col, formula) in formula_rows {
            let key = RowCol::new(row as usize, col as usize);
            match sheet.as_str() {
                s if s == TRANSACTIONS => {
                    txn_formulas.insert(key, formula);
                }
                s if s == CATEGORIES => {
                    cat_formulas.insert(key, formula);
                }
                s if s == AUTO_CAT => {
                    autocat_formulas.insert(key, formula);
                }
                _ => {} // Ignore unknown sheets
            }
        }

        // Load mappings for each sheet
        let txn_mapping = self
            .load_sheet_metadata(TRANSACTIONS)
            .await?
            .unwrap_or_default();
        let cat_mapping = self
            .load_sheet_metadata(CATEGORIES)
            .await?
            .unwrap_or_default();
        let autocat_mapping = self
            .load_sheet_metadata(AUTO_CAT)
            .await?
            .unwrap_or_default();

        Ok(TillerData {
            transactions: Transactions::new(transactions_data, txn_formulas, txn_mapping)?,
            categories: Categories::new(categories_data, cat_formulas, cat_mapping)?,
            auto_cats: AutoCats::new(autocat_data, autocat_formulas, autocat_mapping)?,
        })
    }

    /// Inserts a new transaction into the database.
    pub(crate) async fn _insert_transaction(&self, transaction: &Transaction) -> Res<()> {
        let other_fields_json = if transaction.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&transaction.other_fields)?)
        };

        // Convert empty category to NULL for FK constraint compatibility
        // (uncategorized transactions have no category reference)
        let category = if transaction.category.is_empty() {
            None
        } else {
            Some(&transaction.category)
        };

        sqlx::query(
            r#"INSERT INTO transactions (
                transaction_id, date, description, amount, account, account_number,
                institution, month, week, full_description, account_id, check_number,
                date_added, merchant_name, category_hint, category, note, tags,
                categorized_date, statement, metadata, other_fields, original_order
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&transaction.transaction_id)
        .bind(&transaction.date)
        .bind(&transaction.description)
        .bind(transaction.amount.value().to_f64().unwrap_or(0.0))
        .bind(&transaction.account)
        .bind(&transaction.account_number)
        .bind(&transaction.institution)
        .bind(&transaction.month)
        .bind(&transaction.week)
        .bind(&transaction.full_description)
        .bind(&transaction.account_id)
        .bind(&transaction.check_number)
        .bind(&transaction.date_added)
        .bind(&transaction.merchant_name)
        .bind(&transaction.category_hint)
        .bind(category)
        .bind(&transaction.note)
        .bind(&transaction.tags)
        .bind(&transaction.categorized_date)
        .bind(&transaction.statement)
        .bind(&transaction.metadata)
        .bind(&other_fields_json)
        .bind(transaction.original_order.map(|i| i as i64))
        .execute(&self.pool)
        .await
        .context("Failed to insert transaction")?;

        Ok(())
    }

    /// Updates an existing transaction in the database.
    pub(crate) async fn update_transaction(&self, transaction: &Transaction) -> Res<()> {
        let other_fields_json = if transaction.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&transaction.other_fields)?)
        };

        // Convert empty category to NULL for FK constraint compatibility
        let category = if transaction.category.is_empty() {
            None
        } else {
            Some(&transaction.category)
        };

        sqlx::query(
            r#"UPDATE transactions SET
                date = ?, description = ?, amount = ?, account = ?, account_number = ?,
                institution = ?, month = ?, week = ?, full_description = ?, account_id = ?,
                check_number = ?, date_added = ?, merchant_name = ?, category_hint = ?,
                category = ?, note = ?, tags = ?, categorized_date = ?, statement = ?,
                metadata = ?, other_fields = ?, original_order = ?
            WHERE transaction_id = ?"#,
        )
        .bind(&transaction.date)
        .bind(&transaction.description)
        .bind(transaction.amount.value().to_f64().unwrap_or(0.0))
        .bind(&transaction.account)
        .bind(&transaction.account_number)
        .bind(&transaction.institution)
        .bind(&transaction.month)
        .bind(&transaction.week)
        .bind(&transaction.full_description)
        .bind(&transaction.account_id)
        .bind(&transaction.check_number)
        .bind(&transaction.date_added)
        .bind(&transaction.merchant_name)
        .bind(&transaction.category_hint)
        .bind(category)
        .bind(&transaction.note)
        .bind(&transaction.tags)
        .bind(&transaction.categorized_date)
        .bind(&transaction.statement)
        .bind(&transaction.metadata)
        .bind(&other_fields_json)
        .bind(transaction.original_order.map(|i| i as i64))
        .bind(&transaction.transaction_id)
        .execute(&self.pool)
        .await
        .context("Failed to update transaction")?;

        Ok(())
    }

    /// Retrieves a transaction by its ID.
    pub(crate) async fn get_transaction(&self, id: &str) -> Res<Option<Transaction>> {
        use sqlx::Row;

        let row = sqlx::query(
            r#"SELECT
                transaction_id, date, description, amount, account, account_number,
                institution, month, week, full_description, account_id, check_number,
                date_added, merchant_name, category_hint, category, note, tags,
                categorized_date, statement, metadata, other_fields
            FROM transactions WHERE transaction_id = ?"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get transaction")?;

        match row {
            None => Ok(None),
            Some(r) => {
                let other_fields_json: Option<String> = r.get("other_fields");
                let other_fields: BTreeMap<String, String> = match other_fields_json {
                    Some(json) => serde_json::from_str(&json)?,
                    None => BTreeMap::new(),
                };

                // SQLite may store numeric values as INTEGER or REAL depending on value
                // Use try_get to handle both cases
                let amount_val: f64 = r
                    .try_get::<f64, _>("amount")
                    .or_else(|_| r.try_get::<i64, _>("amount").map(|i| i as f64))
                    .unwrap_or(0.0);

                Ok(Some(Transaction {
                    transaction_id: r.get("transaction_id"),
                    date: r.get("date"),
                    description: r.get("description"),
                    amount: Amount::new(Decimal::from_f64(amount_val).unwrap_or_default()),
                    account: r.get("account"),
                    account_number: r.get("account_number"),
                    institution: r.get("institution"),
                    month: r.get::<Option<String>, _>("month").unwrap_or_default(),
                    week: r.get::<Option<String>, _>("week").unwrap_or_default(),
                    full_description: r
                        .get::<Option<String>, _>("full_description")
                        .unwrap_or_default(),
                    account_id: r.get("account_id"),
                    check_number: r
                        .get::<Option<String>, _>("check_number")
                        .unwrap_or_default(),
                    date_added: r.get::<Option<String>, _>("date_added").unwrap_or_default(),
                    merchant_name: r
                        .get::<Option<String>, _>("merchant_name")
                        .unwrap_or_default(),
                    category_hint: r
                        .get::<Option<String>, _>("category_hint")
                        .unwrap_or_default(),
                    category: r.get::<Option<String>, _>("category").unwrap_or_default(),
                    note: r.get::<Option<String>, _>("note").unwrap_or_default(),
                    tags: r.get::<Option<String>, _>("tags").unwrap_or_default(),
                    categorized_date: r
                        .get::<Option<String>, _>("categorized_date")
                        .unwrap_or_default(),
                    statement: r.get::<Option<String>, _>("statement").unwrap_or_default(),
                    metadata: r.get::<Option<String>, _>("metadata").unwrap_or_default(),
                    other_fields,
                    ..Default::default()
                }))
            }
        }
    }

    /// Inserts a new category into the database. Returns the category name (primary key).
    pub(crate) async fn _insert_category(&self, category: &Category) -> Res<String> {
        let other_fields_json = if category.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&category.other_fields)?)
        };

        sqlx::query(
            r#"INSERT INTO categories (category, category_group, type, hide_from_reports, other_fields, original_order)
            VALUES (?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&category.category)
        .bind(&category.category_group)
        .bind(&category.r#type)
        .bind(&category.hide_from_reports)
        .bind(&other_fields_json)
        .bind(category.original_order.map(|i| i as i64))
        .execute(&self.pool)
        .await
        .context("Failed to insert category")?;

        Ok(category.category.clone())
    }

    /// Updates an existing category in the database.
    ///
    /// The `old_name` parameter is used to find the existing category.
    /// If `new_data.category` differs from `old_name`, the category is renamed.
    /// Due to `ON UPDATE CASCADE` foreign key constraints, renaming a category
    /// automatically updates all references in transactions and autocat.
    ///
    /// Returns the new category name.
    pub(crate) async fn _update_category(
        &self,
        old_name: &str,
        new_data: &Category,
    ) -> Res<String> {
        let other_fields_json = if new_data.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&new_data.other_fields)?)
        };

        let result = sqlx::query(
            r#"UPDATE categories SET
                category = ?, category_group = ?, type = ?, hide_from_reports = ?, other_fields = ?
            WHERE category = ?"#,
        )
        .bind(&new_data.category)
        .bind(&new_data.category_group)
        .bind(&new_data.r#type)
        .bind(&new_data.hide_from_reports)
        .bind(&other_fields_json)
        .bind(old_name)
        .execute(&self.pool)
        .await
        .context("Failed to update category")?;

        if result.rows_affected() == 0 {
            bail!("Category '{}' not found", old_name);
        }

        Ok(new_data.category.clone())
    }

    /// Retrieves a category by its name (primary key).
    pub(crate) async fn _get_category(&self, name: &str) -> Res<Option<Category>> {
        use sqlx::Row;

        let row = sqlx::query(
            r#"SELECT category, category_group, type, hide_from_reports, other_fields, original_order
            FROM categories WHERE category = ?"#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get category")?;

        match row {
            None => Ok(None),
            Some(r) => {
                let other_fields_json: Option<String> = r.get("other_fields");
                let other_fields: BTreeMap<String, String> = match other_fields_json {
                    Some(json) => serde_json::from_str(&json)?,
                    None => BTreeMap::new(),
                };

                Ok(Some(Category {
                    category: r.get("category"),
                    category_group: r
                        .get::<Option<String>, _>("category_group")
                        .unwrap_or_default(),
                    r#type: r.get::<Option<String>, _>("type").unwrap_or_default(),
                    hide_from_reports: r
                        .get::<Option<String>, _>("hide_from_reports")
                        .unwrap_or_default(),
                    other_fields,
                    original_order: r.get::<Option<u64>, _>("original_order"),
                }))
            }
        }
    }

    /// Inserts a new autocat rule into the database. Returns the primary key ID.
    pub(crate) async fn _insert_autocat(&self, autocat: &AutoCat) -> Res<u64> {
        let other_fields_json = if autocat.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&autocat.other_fields)?)
        };

        // Convert empty category to NULL for FK constraint compatibility
        let category = if autocat.category.is_empty() {
            None
        } else {
            Some(&autocat.category)
        };

        let result = sqlx::query(
            r#"INSERT INTO autocat (
                category, description, description_contains, account_contains,
                institution_contains, amount_min, amount_max, amount_equals,
                description_equals, description_full, full_description_contains,
                amount_contains, other_fields, original_order
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(category)
        .bind(&autocat.description)
        .bind(&autocat.description_contains)
        .bind(&autocat.account_contains)
        .bind(&autocat.institution_contains)
        .bind(
            autocat
                .amount_min
                .as_ref()
                .map(|a| a.value().to_f64().unwrap_or(0.0)),
        )
        .bind(
            autocat
                .amount_max
                .as_ref()
                .map(|a| a.value().to_f64().unwrap_or(0.0)),
        )
        .bind(
            autocat
                .amount_equals
                .as_ref()
                .map(|a| a.value().to_f64().unwrap_or(0.0)),
        )
        .bind(&autocat.description_equals)
        .bind(&autocat.description_full)
        .bind(&autocat.full_description_contains)
        .bind(&autocat.amount_contains)
        .bind(&other_fields_json)
        .bind(autocat.original_order.map(|i| i as i64))
        .execute(&self.pool)
        .await
        .context("Failed to insert autocat")?;

        Ok(result.last_insert_rowid() as u64)
    }

    /// Updates an existing autocat rule in the database. Returns the primary key ID.
    pub(crate) async fn _update_autocat(&self, autocat: &_Row<AutoCat>) -> Res<u64> {
        let other_fields_json = if autocat.row.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&autocat.row.other_fields)?)
        };

        // Convert empty category to NULL for FK constraint compatibility
        let category = if autocat.row.category.is_empty() {
            None
        } else {
            Some(&autocat.row.category)
        };

        sqlx::query(
            r#"UPDATE autocat SET
                category = ?, description = ?, description_contains = ?, account_contains = ?,
                institution_contains = ?, amount_min = ?, amount_max = ?, amount_equals = ?,
                description_equals = ?, description_full = ?, full_description_contains = ?,
                amount_contains = ?, other_fields = ?
            WHERE id = ?"#,
        )
        .bind(category)
        .bind(&autocat.row.description)
        .bind(&autocat.row.description_contains)
        .bind(&autocat.row.account_contains)
        .bind(&autocat.row.institution_contains)
        .bind(
            autocat
                .row
                .amount_min
                .as_ref()
                .map(|a| a.value().to_f64().unwrap_or(0.0)),
        )
        .bind(
            autocat
                .row
                .amount_max
                .as_ref()
                .map(|a| a.value().to_f64().unwrap_or(0.0)),
        )
        .bind(
            autocat
                .row
                .amount_equals
                .as_ref()
                .map(|a| a.value().to_f64().unwrap_or(0.0)),
        )
        .bind(&autocat.row.description_equals)
        .bind(&autocat.row.description_full)
        .bind(&autocat.row.full_description_contains)
        .bind(&autocat.row.amount_contains)
        .bind(&other_fields_json)
        .bind(autocat.id as i64)
        .execute(&self.pool)
        .await
        .context("Failed to update autocat")?;

        Ok(autocat.id)
    }

    /// Retrieves an autocat rule by its ID.
    pub(crate) async fn _get_autocat(&self, id: &str) -> Res<Option<_Row<AutoCat>>> {
        use sqlx::Row;

        let id_num: i64 = id.parse().context("Invalid autocat ID")?;

        let row = sqlx::query(
            r#"SELECT id, category, description, description_contains, account_contains,
                institution_contains, amount_min, amount_max, amount_equals,
                description_equals, description_full, full_description_contains,
                amount_contains, other_fields, original_order
            FROM autocat WHERE id = ?"#,
        )
        .bind(id_num)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get autocat")?;

        match row {
            None => Ok(None),
            Some(r) => {
                let other_fields_json: Option<String> = r.get("other_fields");
                let other_fields: BTreeMap<String, String> = match other_fields_json {
                    Some(json) => serde_json::from_str(&json)?,
                    None => BTreeMap::new(),
                };

                let amount_min: Option<String> = r.get("amount_min");
                let amount_max: Option<String> = r.get("amount_max");
                let amount_equals: Option<String> = r.get("amount_equals");

                Ok(Some(_Row {
                    id: r.get::<i64, _>("id") as u64,
                    row: AutoCat {
                        category: r.get::<Option<String>, _>("category").unwrap_or_default(),
                        description: r
                            .get::<Option<String>, _>("description")
                            .unwrap_or_default(),
                        description_contains: r
                            .get::<Option<String>, _>("description_contains")
                            .unwrap_or_default(),
                        account_contains: r
                            .get::<Option<String>, _>("account_contains")
                            .unwrap_or_default(),
                        institution_contains: r
                            .get::<Option<String>, _>("institution_contains")
                            .unwrap_or_default(),
                        amount_min: amount_min.and_then(|v| v.parse().ok()),
                        amount_max: amount_max.and_then(|v| v.parse().ok()),
                        amount_equals: amount_equals.and_then(|v| v.parse().ok()),
                        description_equals: r
                            .get::<Option<String>, _>("description_equals")
                            .unwrap_or_default(),
                        description_full: r
                            .get::<Option<String>, _>("description_full")
                            .unwrap_or_default(),
                        full_description_contains: r
                            .get::<Option<String>, _>("full_description_contains")
                            .unwrap_or_default(),
                        amount_contains: r
                            .get::<Option<String>, _>("amount_contains")
                            .unwrap_or_default(),
                        other_fields,
                        original_order: r.get::<Option<u64>, _>("original_order"),
                    },
                }))
            }
        }
    }

    // /// Returns a reference to the underlying connection pool.
    // fn pool(&self) -> &SqlitePool {
    //     &self.pool
    // }

    /// Creates the schema_version table and inserts version 0. This establishes the invariant
    /// that schema_version always exists, allowing migration logic to work uniformly.
    async fn bootstrap(&self) -> Res<()> {
        sqlx::query(
            "CREATE TABLE schema_version (
                version INTEGER NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create schema_version table")?;

        sqlx::query("INSERT INTO schema_version (version) VALUES (0)")
            .execute(&self.pool)
            .await
            .context("Failed to insert initial schema version")?;

        Ok(())
    }

    /// Returns the current schema version from the database.
    async fn schema_version(&self) -> Res<i32> {
        let row: (i32,) = sqlx::query_as("SELECT MAX(version) FROM schema_version")
            .fetch_one(&self.pool)
            .await
            .context("Failed to query schema version")?;
        Ok(row.0)
    }

    /// Runs migrations to bring the database to CURRENT_VERSION.
    async fn migrate(&self) -> Res<()> {
        let current = self.schema_version().await?;
        migrations::run(&self.pool, current, CURRENT_VERSION).await
    }

    /// Deletes a transaction by its ID. Used for testing gap detection.
    pub(crate) async fn _delete_transaction(&self, transaction_id: &str) -> Res<()> {
        sqlx::query("DELETE FROM transactions WHERE transaction_id = ?")
            .bind(transaction_id)
            .execute(&self.pool)
            .await
            .context("Failed to delete transaction")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AutoCat, AutoCats, Categories, Category, Transaction, Transactions};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bootstrap_creates_schema_version_table() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");

        let db = Db::init(&db_path).await.unwrap();

        // Verify the database file was created
        assert!(db_path.exists());

        // Verify schema_version table exists and contains a valid version
        // (>= 0 because migrations may run during init)
        let version = db.schema_version().await.unwrap();
        assert!(version >= 0);
    }

    #[tokio::test]
    async fn test_save_tiller_data() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Create TillerData with actual data
        let transactions = Transactions::parse(
            vec![
                vec![
                    "Transaction ID",
                    "Date",
                    "Description",
                    "Amount",
                    "Account",
                    "Account #",
                    "Institution",
                    "Account ID",
                ],
                vec![
                    "txn-001",
                    "2025-01-15",
                    "Coffee Shop",
                    "-4.50",
                    "Checking",
                    "1234",
                    "Test Bank",
                    "acct-001",
                ],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let categories = Categories::parse(
            vec![
                vec!["Category", "Group", "Type", "Hide From Reports"],
                vec!["Groceries", "Food", "Expense", ""],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let auto_cats = AutoCats::parse(
            vec![
                vec!["Category", "Description Contains"],
                vec!["Groceries", "grocery"],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let data = TillerData {
            transactions,
            categories,
            auto_cats,
        };

        db.save_tiller_data(&data).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_tiller_data() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert sheet_metadata for all three sheets (required for _get_tiller_data)
        let metadata = vec![
            ("Transactions", "Transaction ID", 0),
            ("Transactions", "Date", 1),
            ("Transactions", "Description", 2),
            ("Transactions", "Amount", 3),
            ("Transactions", "Account", 4),
            ("Transactions", "Account #", 5),
            ("Transactions", "Institution", 6),
            ("Transactions", "Account ID", 7),
            ("Categories", "Category", 0),
            ("Categories", "Group", 1),
            ("Categories", "Type", 2),
            ("Categories", "Hide From Reports", 3),
            ("AutoCat", "Category", 0),
            ("AutoCat", "Description Contains", 1),
        ];
        for (sheet, header, order) in metadata {
            sqlx::query(
                r#"INSERT INTO sheet_metadata (sheet, column_name, header_name, "order")
                   VALUES (?, ?, ?, ?)"#,
            )
            .bind(sheet)
            .bind(header.to_lowercase().replace(' ', "_").replace('#', ""))
            .bind(header)
            .bind(order)
            .execute(&db.pool)
            .await
            .unwrap();
        }

        // Insert test data directly with sqlx
        sqlx::query(
            "INSERT INTO transactions (transaction_id, date, description, amount, account, account_number, institution, account_id)
             VALUES ('txn-001', '2025-01-15', 'Coffee Shop', -4.50, 'Checking', '1234', 'Test Bank', 'acct-001')"
        )
        .execute(&db.pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO categories (category, category_group, type, hide_from_reports)
             VALUES ('Groceries', 'Food', 'Expense', '')",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO autocat (category, description_contains)
             VALUES ('Groceries', 'grocery')",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        let data = db.get_tiller_data().await.unwrap();

        // Verify retrieved data
        assert_eq!(data.transactions.data().len(), 1);
        assert_eq!(data.transactions.data()[0].transaction_id, "txn-001");
    }

    #[tokio::test]
    async fn test_insert_transaction() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let mut transaction = Transaction::default();
        transaction.transaction_id = "txn-001".to_string();
        transaction.date = "2025-01-15".to_string();
        transaction.description = "Coffee Shop".to_string();
        transaction.account = "Checking".to_string();
        transaction.account_number = "1234".to_string();
        transaction.institution = "Test Bank".to_string();
        transaction.account_id = "acct-001".to_string();

        db._insert_transaction(&transaction).await.unwrap();

        // Verify by querying directly
        let row: (String,) = sqlx::query_as(
            "SELECT transaction_id FROM transactions WHERE transaction_id = 'txn-001'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(row.0, "txn-001");
    }

    #[tokio::test]
    async fn test_update_transaction() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert a transaction directly
        sqlx::query(
            "INSERT INTO transactions (transaction_id, date, description, amount, account, account_number, institution, account_id)
             VALUES ('txn-001', '2025-01-15', 'Coffee Shop', -4.50, 'Checking', '1234', 'Test Bank', 'acct-001')"
        )
        .execute(&db.pool)
        .await
        .unwrap();

        // Update via the method
        let mut transaction = Transaction::default();
        transaction.transaction_id = "txn-001".to_string();
        transaction.date = "2025-01-15".to_string();
        transaction.description = "Updated Description".to_string();
        transaction.account = "Checking".to_string();
        transaction.account_number = "1234".to_string();
        transaction.institution = "Test Bank".to_string();
        transaction.account_id = "acct-001".to_string();

        db.update_transaction(&transaction).await.unwrap();

        // Verify the update
        let row: (String,) =
            sqlx::query_as("SELECT description FROM transactions WHERE transaction_id = 'txn-001'")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(row.0, "Updated Description");
    }

    #[tokio::test]
    async fn test_get_transaction() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert a transaction directly
        sqlx::query(
            "INSERT INTO transactions (transaction_id, date, description, amount, account, account_number, institution, account_id)
             VALUES ('txn-001', '2025-01-15', 'Coffee Shop', -4.50, 'Checking', '1234', 'Test Bank', 'acct-001')"
        )
        .execute(&db.pool)
        .await
        .unwrap();

        let transaction = db.get_transaction("txn-001").await.unwrap();

        assert!(transaction.is_some());
        let transaction = transaction.unwrap();
        assert_eq!(transaction.transaction_id, "txn-001");
        assert_eq!(transaction.description, "Coffee Shop");
    }

    #[tokio::test]
    async fn test_insert_category() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let mut category = Category::default();
        category.category = "Groceries".to_string();
        category.category_group = "Food".to_string();
        category.r#type = "Expense".to_string();
        category.hide_from_reports = "".to_string();

        let name = db._insert_category(&category).await.unwrap();
        assert_eq!(name, "Groceries");

        // Verify by querying directly
        let row: (String, String) =
            sqlx::query_as("SELECT category, category_group FROM categories WHERE category = ?")
                .bind(&name)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(row.0, "Groceries");
        assert_eq!(row.1, "Food");
    }

    #[tokio::test]
    async fn test_update_category() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert a category directly
        sqlx::query(
            "INSERT INTO categories (category, category_group, type, hide_from_reports)
             VALUES ('Groceries', 'Food', 'Expense', '')",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        // Update via the method (renaming from "Groceries" to "Updated Groceries")
        let mut new_data = Category::default();
        new_data.category = "Updated Groceries".to_string();
        new_data.category_group = "Updated Food".to_string();
        new_data.r#type = "Expense".to_string();
        new_data.hide_from_reports = "".to_string();

        db._update_category("Groceries", &new_data).await.unwrap();

        // Verify the update by querying with the new name
        let (name, group): (String, String) =
            sqlx::query_as("SELECT category, category_group FROM categories WHERE category = ?")
                .bind("Updated Groceries")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(name, "Updated Groceries");
        assert_eq!(group, "Updated Food");

        // Verify old name no longer exists
        let old_exists: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM categories WHERE category = ?")
                .bind("Groceries")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(old_exists.0, 0);
    }

    #[tokio::test]
    async fn test_get_category() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert a category directly
        sqlx::query(
            "INSERT INTO categories (category, category_group, type, hide_from_reports)
             VALUES ('Groceries', 'Food', 'Expense', '')",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        // Get by category name (primary key)
        let result = db._get_category("Groceries").await.unwrap();

        assert!(result.is_some());
        let category = result.unwrap();
        assert_eq!(category.category, "Groceries");
        assert_eq!(category.category_group, "Food");
    }

    #[tokio::test]
    async fn test_insert_autocat() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert category first (FK constraint requires it)
        sqlx::query("INSERT INTO categories (category) VALUES ('Groceries')")
            .execute(&db.pool)
            .await
            .unwrap();

        let mut autocat = AutoCat::default();
        autocat.category = "Groceries".to_string();
        autocat.description_contains = "grocery".to_string();

        let id = db._insert_autocat(&autocat).await.unwrap();

        // Verify by querying directly
        let row: (String, String) =
            sqlx::query_as("SELECT category, description_contains FROM autocat WHERE id = ?")
                .bind(id as i64)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(row.0, "Groceries");
        assert_eq!(row.1, "grocery");
    }

    #[tokio::test]
    async fn test_update_autocat() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert categories first (FK constraint requires them)
        sqlx::query("INSERT INTO categories (category) VALUES ('Groceries')")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO categories (category) VALUES ('Updated Groceries')")
            .execute(&db.pool)
            .await
            .unwrap();

        // Insert an autocat rule directly
        sqlx::query(
            "INSERT INTO autocat (category, description_contains)
             VALUES ('Groceries', 'grocery')",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        // Get the inserted ID
        let (id,): (i64,) = sqlx::query_as("SELECT last_insert_rowid()")
            .fetch_one(&db.pool)
            .await
            .unwrap();

        // Update via the method (changing category from Groceries to Updated Groceries)
        let mut autocat = AutoCat::default();
        autocat.category = "Updated Groceries".to_string();
        autocat.description_contains = "updated grocery".to_string();

        let row = _Row {
            id: id as u64,
            row: autocat,
        };

        db._update_autocat(&row).await.unwrap();

        // Verify the update
        let (category, desc): (String, String) =
            sqlx::query_as("SELECT category, description_contains FROM autocat WHERE id = ?")
                .bind(id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(category, "Updated Groceries");
        assert_eq!(desc, "updated grocery");
    }

    #[tokio::test]
    async fn test_get_autocat() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Insert category first (FK constraint requires it)
        sqlx::query("INSERT INTO categories (category) VALUES ('Groceries')")
            .execute(&db.pool)
            .await
            .unwrap();

        // Insert an autocat rule directly
        sqlx::query(
            "INSERT INTO autocat (category, description_contains)
             VALUES ('Groceries', 'grocery')",
        )
        .execute(&db.pool)
        .await
        .unwrap();

        // Get the inserted ID
        let (id,): (i64,) = sqlx::query_as("SELECT last_insert_rowid()")
            .fetch_one(&db.pool)
            .await
            .unwrap();

        let result = db._get_autocat(&id.to_string()).await.unwrap();

        assert!(result.is_some());
        let row = result.unwrap();
        assert_eq!(row.id, id as u64);
        assert_eq!(row.row.category, "Groceries");
        assert_eq!(row.row.description_contains, "grocery");
    }

    // --- Edge case tests ---

    #[tokio::test]
    async fn test_get_transaction_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let result = db.get_transaction("non-existent-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_category_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Look up a category that doesn't exist
        let result = db._get_category("Non-existent Category").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_autocat_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let result = db._get_autocat("99999").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_insert_transaction_with_other_fields() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let mut transaction = Transaction::default();
        transaction.transaction_id = "txn-other".to_string();
        transaction.date = "2025-01-15".to_string();
        transaction.description = "Test".to_string();
        transaction.account = "Checking".to_string();
        transaction.account_number = "1234".to_string();
        transaction.institution = "Test Bank".to_string();
        transaction.account_id = "acct-001".to_string();
        transaction
            .other_fields
            .insert("Custom Column".to_string(), "custom value".to_string());

        db._insert_transaction(&transaction).await.unwrap();

        // Retrieve and verify other_fields was stored
        let retrieved = db.get_transaction("txn-other").await.unwrap().unwrap();
        assert_eq!(
            retrieved.other_fields.get("Custom Column"),
            Some(&"custom value".to_string())
        );
    }

    #[tokio::test]
    async fn test_insert_category_with_other_fields() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let mut category = Category::default();
        category.category = "Test Category".to_string();
        category
            .other_fields
            .insert("Extra Field".to_string(), "extra value".to_string());

        let name = db._insert_category(&category).await.unwrap();
        assert_eq!(name, "Test Category");

        // Retrieve and verify other_fields was stored
        let retrieved = db._get_category(&name).await.unwrap().unwrap();
        assert_eq!(
            retrieved.other_fields.get("Extra Field"),
            Some(&"extra value".to_string())
        );
    }

    // Note: test_insert_transaction_with_original_order is deferred until
    // original_order field is added to the Transaction model struct.

    #[tokio::test]
    async fn test_insert_duplicate_transaction_id_fails() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let mut transaction = Transaction::default();
        transaction.transaction_id = "txn-dup".to_string();
        transaction.date = "2025-01-15".to_string();
        transaction.description = "First".to_string();
        transaction.account = "Checking".to_string();
        transaction.account_number = "1234".to_string();
        transaction.institution = "Test Bank".to_string();
        transaction.account_id = "acct-001".to_string();

        db._insert_transaction(&transaction).await.unwrap();

        // Try to insert duplicate - should fail
        transaction.description = "Second".to_string();
        let result = db._insert_transaction(&transaction).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_insert_duplicate_category_name_fails() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let mut category = Category::default();
        category.category = "Duplicate".to_string();

        db._insert_category(&category).await.unwrap();

        // Try to insert duplicate - should fail due to UNIQUE constraint
        let result = db._insert_category(&category).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_save_tiller_data_saves_formulas() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Create TillerData with formulas in "Custom Column"
        // The formula data differs from value data to trigger formula detection
        let transactions = Transactions::parse(
            vec![
                vec![
                    "Transaction ID",
                    "Date",
                    "Description",
                    "Amount",
                    "Account",
                    "Account #",
                    "Institution",
                    "Account ID",
                    "Custom Column",
                ],
                vec![
                    "txn-001",
                    "2025-01-15",
                    "Coffee Shop",
                    "-4.50",
                    "Checking",
                    "1234",
                    "Test Bank",
                    "acct-001",
                    "4.50", // This is the computed value
                ],
            ],
            // Formula data: same as values except Custom Column has a formula
            vec![
                vec![
                    "Transaction ID",
                    "Date",
                    "Description",
                    "Amount",
                    "Account",
                    "Account #",
                    "Institution",
                    "Account ID",
                    "Custom Column",
                ],
                vec![
                    "txn-001",
                    "2025-01-15",
                    "Coffee Shop",
                    "-4.50",
                    "Checking",
                    "1234",
                    "Test Bank",
                    "acct-001",
                    "=ABS(D2)", // This is the formula
                ],
            ],
        )
        .unwrap();

        // Verify the transactions object detected the formula
        assert!(
            !transactions.formulas().is_empty(),
            "Expected formulas to be detected in Transactions"
        );

        let categories = Categories::parse(
            vec![
                vec!["Category", "Group", "Type", "Hide From Reports"],
                vec!["Groceries", "Food", "Expense", ""],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let auto_cats = AutoCats::parse(
            vec![
                vec!["Category", "Description Contains"],
                vec!["Groceries", "grocery"],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let data = TillerData {
            transactions,
            categories,
            auto_cats,
        };

        // Save the data (should save formulas too)
        db.save_tiller_data(&data).await.unwrap();

        // Verify formulas were saved by querying the formulas table directly
        let formula_rows: Vec<(String, i64, i64, String)> = sqlx::query_as(
            "SELECT sheet, row, col, formula FROM formulas ORDER BY sheet, row, col",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();

        assert!(
            !formula_rows.is_empty(),
            "Expected formulas to be saved in the formulas table, but found none"
        );

        // Verify the specific formula was saved
        // Row 0 (first data row, 0-indexed), Col 8 (Custom Column index)
        let expected = (
            "Transactions".to_string(),
            0_i64,
            8_i64,
            "=ABS(D2)".to_string(),
        );
        assert!(
            formula_rows.contains(&expected),
            "Expected formula {:?} not found in {:?}",
            expected,
            formula_rows
        );
    }

    #[tokio::test]
    async fn test_get_tiller_data_loads_formulas() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Create TillerData with formulas in "Custom Column"
        let transactions = Transactions::parse(
            vec![
                vec![
                    "Transaction ID",
                    "Date",
                    "Description",
                    "Amount",
                    "Account",
                    "Account #",
                    "Institution",
                    "Account ID",
                    "Custom Column",
                ],
                vec![
                    "txn-001",
                    "2025-01-15",
                    "Coffee Shop",
                    "-4.50",
                    "Checking",
                    "1234",
                    "Test Bank",
                    "acct-001",
                    "4.50",
                ],
            ],
            vec![
                vec![
                    "Transaction ID",
                    "Date",
                    "Description",
                    "Amount",
                    "Account",
                    "Account #",
                    "Institution",
                    "Account ID",
                    "Custom Column",
                ],
                vec![
                    "txn-001",
                    "2025-01-15",
                    "Coffee Shop",
                    "-4.50",
                    "Checking",
                    "1234",
                    "Test Bank",
                    "acct-001",
                    "=ABS(D2)",
                ],
            ],
        )
        .unwrap();

        // Verify the transactions object detected the formula
        assert!(
            !transactions.formulas().is_empty(),
            "Expected formulas to be detected in Transactions"
        );

        let categories = Categories::parse(
            vec![
                vec!["Category", "Group", "Type", "Hide From Reports"],
                vec!["Groceries", "Food", "Expense", ""],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let auto_cats = AutoCats::parse(
            vec![
                vec!["Category", "Description Contains"],
                vec!["Groceries", "grocery"],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let data = TillerData {
            transactions,
            categories,
            auto_cats,
        };

        // Save the data (including formulas)
        db.save_tiller_data(&data).await.unwrap();

        // Load the data back
        let loaded_data = db.get_tiller_data().await.unwrap();

        // Verify formulas were loaded
        assert!(
            !loaded_data.transactions.formulas().is_empty(),
            "Expected formulas to be loaded from database, but found none"
        );

        // Verify the specific formula was loaded
        use crate::model::RowCol;
        let expected_key = RowCol::new(0, 8);
        let formula = loaded_data.transactions.formulas().get(&expected_key);
        assert_eq!(
            formula,
            Some(&"=ABS(D2)".to_string()),
            "Expected formula =ABS(D2) at position (0, 8)"
        );
    }

    /// Test that save_tiller_data saves the header mapping to the sheet_metadata table.
    /// This test is expected to FAIL until we implement saving mappings.
    #[tokio::test]
    async fn test_save_tiller_data_saves_mapping_to_sheet_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Create TillerData with specific headers including a custom column
        let transactions = Transactions::parse(
            vec![
                vec![
                    "Transaction ID",
                    "Date",
                    "Description",
                    "Amount",
                    "Account",
                    "Account #",
                    "Institution",
                    "Account ID",
                    "My Custom Column", // Custom column that must be preserved
                ],
                vec![
                    "txn-001",
                    "2025-01-15",
                    "Coffee Shop",
                    "-4.50",
                    "Checking",
                    "1234",
                    "Test Bank",
                    "acct-001",
                    "custom value",
                ],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let categories = Categories::parse(
            vec![
                vec!["Category", "Group", "Type", "Hide From Reports"],
                vec!["Groceries", "Food", "Expense", ""],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let auto_cats = AutoCats::parse(
            vec![
                vec!["Category", "Description Contains"],
                vec!["Groceries", "grocery"],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let data = TillerData {
            transactions,
            categories,
            auto_cats,
        };

        // Save the data
        db.save_tiller_data(&data).await.unwrap();

        // Query sheet_metadata to verify headers were saved
        let metadata_rows: Vec<(String, String, String, i64)> = sqlx::query_as(
            r#"SELECT sheet, column_name, header_name, "order"
               FROM sheet_metadata
               WHERE sheet = 'Transactions'
               ORDER BY "order""#,
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();

        // This assertion will FAIL because we're not saving to sheet_metadata
        assert!(
            !metadata_rows.is_empty(),
            "Expected sheet_metadata to contain header mappings for Transactions, but it was empty"
        );

        // Verify the custom column is present
        let custom_col = metadata_rows
            .iter()
            .find(|(_, _, header, _)| header == "My Custom Column");
        assert!(
            custom_col.is_some(),
            "Expected 'My Custom Column' to be saved in sheet_metadata"
        );

        // Verify the order is correct (custom column should be at index 8)
        let (_, _, _, order) = custom_col.unwrap();
        assert_eq!(
            *order, 8,
            "Expected 'My Custom Column' to be at order 8, but was at {order}"
        );
    }

    /// Test that _get_tiller_data loads the mapping from sheet_metadata.
    /// This test is expected to FAIL until we implement loading mappings.
    #[tokio::test]
    async fn test_get_tiller_data_loads_mapping_from_sheet_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        // Manually insert sheet_metadata for all three sheets (Transactions with a custom column)
        let headers = vec![
            ("Transactions", "transaction_id", "Transaction ID", 0),
            ("Transactions", "date", "Date", 1),
            ("Transactions", "description", "Description", 2),
            ("Transactions", "amount", "Amount", 3),
            ("Transactions", "account", "Account", 4),
            ("Transactions", "account_number", "Account #", 5),
            ("Transactions", "institution", "Institution", 6),
            ("Transactions", "account_id", "Account ID", 7),
            ("Transactions", "my_custom_column", "My Custom Column", 8),
            ("Categories", "category", "Category", 0),
            ("Categories", "group", "Group", 1),
            ("Categories", "type", "Type", 2),
            ("Categories", "hide_from_reports", "Hide From Reports", 3),
            ("AutoCat", "category", "Category", 0),
            ("AutoCat", "description_contains", "Description Contains", 1),
        ];

        for (sheet, col_name, header_name, order) in &headers {
            sqlx::query(
                r#"INSERT INTO sheet_metadata (sheet, column_name, header_name, "order")
                   VALUES (?, ?, ?, ?)"#,
            )
            .bind(sheet)
            .bind(col_name)
            .bind(header_name)
            .bind(order)
            .execute(&db.pool)
            .await
            .unwrap();
        }

        // Insert a transaction with the custom column value in other_fields
        sqlx::query(
            r#"INSERT INTO transactions
               (transaction_id, date, description, amount, account, account_number,
                institution, account_id, other_fields)
               VALUES ('txn-001', '2025-01-15', 'Coffee Shop', -4.50, 'Checking', '1234',
                       'Test Bank', 'acct-001', '{"My Custom Column": "custom value"}')"#,
        )
        .execute(&db.pool)
        .await
        .unwrap();

        // Load the data
        let loaded_data = db.get_tiller_data().await.unwrap();

        // Get the mapping from loaded transactions
        let mapping = loaded_data.transactions.mapping();

        // This assertion will FAIL because we're using Mapping::default()
        assert!(
            !mapping.headers().is_empty(),
            "Expected mapping to be loaded from sheet_metadata, but got empty mapping (Mapping::default())"
        );

        // Verify the custom column header is present
        let headers: Vec<&str> = mapping.headers().iter().map(|h| h.as_ref()).collect();
        assert!(
            headers.contains(&"My Custom Column"),
            "Expected mapping to contain 'My Custom Column', but got: {:?}",
            headers
        );

        // Verify the column count matches what we inserted
        assert_eq!(
            mapping.headers().len(),
            9,
            "Expected 9 headers (including custom column), but got {}",
            mapping.headers().len()
        );
    }
}
