//! This module is responsible for reading, writing and managing the SQLite database. The internal
//! details of SQLite interaction are hidden while broader functions are exposed.

mod migrations;

use crate::model::{Amount, AutoCat, Category, TillerData, Transaction};
use crate::Result;
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
    pub(crate) async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            bail!("SQLite database not found at {}", path.display());
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .context("Failed to parse SQLite connection string")?
            .create_if_missing(false);

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
    pub(crate) async fn init(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            bail!("SQLite database already exists at {}", path.display());
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))
            .context("Failed to parse SQLite connection string")?
            .create_if_missing(true);

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
    pub(crate) fn count_transactions(&self) -> Result<u64> {
        // TODO: Return the actual count of transaction rows
        Ok(100)
    }

    /// Saves TillerData into the database.
    /// - Transactions: upsert (insert new, update existing, delete removed)
    /// - Categories: delete all, then insert all
    /// - AutoCat: delete all, then insert all
    pub(crate) async fn _save_tiller_data(&self, data: &TillerData) -> Result<()> {
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
                self._update_transaction(transaction).await?;
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

        Ok(())
    }

    /// Retrieves all data from the database as TillerData.
    pub(crate) async fn _get_tiller_data(&self) -> Result<TillerData> {
        use crate::model::{AutoCats, Categories, Transactions};
        use sqlx::Row;

        // Query all transactions
        let rows = sqlx::query(
            r#"SELECT
                transaction_id, date, description, amount, account, account_number,
                institution, month, week, full_description, account_id, check_number,
                date_added, merchant_name, category_hint, category, note, tags,
                categorized_date, statement, metadata, other_fields
            FROM transactions"#,
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
                ..Default::default()
            });
        }

        // Query all categories
        let rows = sqlx::query(
            "SELECT id, category, category_group, type, hide_from_reports, other_fields FROM categories",
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
                _type: r.get::<Option<String>, _>("type").unwrap_or_default(),
                hide_from_reports: r
                    .get::<Option<String>, _>("hide_from_reports")
                    .unwrap_or_default(),
                other_fields,
            });
        }

        // Query all autocat rules
        let rows = sqlx::query(
            r#"SELECT id, category, description, description_contains, account_contains,
                institution_contains, amount_min, amount_max, amount_equals,
                description_equals, description_full, full_description_contains,
                amount_contains, other_fields
            FROM autocat"#,
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

            let amount_min: Option<f64> = r.get("amount_min");
            let amount_max: Option<f64> = r.get("amount_max");
            let amount_equals: Option<f64> = r.get("amount_equals");

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
                amount_min: amount_min.and_then(|v| Decimal::from_f64(v).map(Amount::new)),
                amount_max: amount_max.and_then(|v| Decimal::from_f64(v).map(Amount::new)),
                amount_equals: amount_equals.and_then(|v| Decimal::from_f64(v).map(Amount::new)),
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
            });
        }

        Ok(TillerData {
            transactions: Transactions::_from_data(transactions_data),
            categories: Categories::_from_data(categories_data),
            auto_cats: AutoCats::_from_data(autocat_data),
        })
    }

    /// Inserts a new transaction into the database.
    pub(crate) async fn _insert_transaction(&self, transaction: &Transaction) -> Result<()> {
        let other_fields_json = if transaction.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&transaction.other_fields)?)
        };

        sqlx::query(
            r#"INSERT INTO transactions (
                transaction_id, date, description, amount, account, account_number,
                institution, month, week, full_description, account_id, check_number,
                date_added, merchant_name, category_hint, category, note, tags,
                categorized_date, statement, metadata, other_fields
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
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
        .bind(&transaction.category)
        .bind(&transaction.note)
        .bind(&transaction.tags)
        .bind(&transaction.categorized_date)
        .bind(&transaction.statement)
        .bind(&transaction.metadata)
        .bind(&other_fields_json)
        .execute(&self.pool)
        .await
        .context("Failed to insert transaction")?;

        Ok(())
    }

    /// Updates an existing transaction in the database.
    pub(crate) async fn _update_transaction(&self, transaction: &Transaction) -> Result<()> {
        let other_fields_json = if transaction.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&transaction.other_fields)?)
        };

        sqlx::query(
            r#"UPDATE transactions SET
                date = ?, description = ?, amount = ?, account = ?, account_number = ?,
                institution = ?, month = ?, week = ?, full_description = ?, account_id = ?,
                check_number = ?, date_added = ?, merchant_name = ?, category_hint = ?,
                category = ?, note = ?, tags = ?, categorized_date = ?, statement = ?,
                metadata = ?, other_fields = ?
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
        .bind(&transaction.category)
        .bind(&transaction.note)
        .bind(&transaction.tags)
        .bind(&transaction.categorized_date)
        .bind(&transaction.statement)
        .bind(&transaction.metadata)
        .bind(&other_fields_json)
        .bind(&transaction.transaction_id)
        .execute(&self.pool)
        .await
        .context("Failed to update transaction")?;

        Ok(())
    }

    /// Retrieves a transaction by its ID.
    pub(crate) async fn _get_transaction(&self, id: &str) -> Result<Option<Transaction>> {
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

    /// Inserts a new category into the database. Returns the primary key ID.
    pub(crate) async fn _insert_category(&self, category: &Category) -> Result<u64> {
        let other_fields_json = if category.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&category.other_fields)?)
        };

        let result = sqlx::query(
            r#"INSERT INTO categories (category, category_group, type, hide_from_reports, other_fields)
            VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(&category.category)
        .bind(&category.category_group)
        .bind(&category._type)
        .bind(&category.hide_from_reports)
        .bind(&other_fields_json)
        .execute(&self.pool)
        .await
        .context("Failed to insert category")?;

        Ok(result.last_insert_rowid() as u64)
    }

    /// Updates an existing category in the database. Returns the primary key ID.
    pub(crate) async fn _update_category(&self, category: &_Row<Category>) -> Result<u64> {
        let other_fields_json = if category.row.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&category.row.other_fields)?)
        };

        sqlx::query(
            r#"UPDATE categories SET
                category = ?, category_group = ?, type = ?, hide_from_reports = ?, other_fields = ?
            WHERE id = ?"#,
        )
        .bind(&category.row.category)
        .bind(&category.row.category_group)
        .bind(&category.row._type)
        .bind(&category.row.hide_from_reports)
        .bind(&other_fields_json)
        .bind(category.id as i64)
        .execute(&self.pool)
        .await
        .context("Failed to update category")?;

        Ok(category.id)
    }

    /// Retrieves a category by its ID.
    pub(crate) async fn _get_category(&self, id: &str) -> Result<Option<_Row<Category>>> {
        use sqlx::Row;

        let id_num: i64 = id.parse().context("Invalid category ID")?;

        let row = sqlx::query(
            r#"SELECT id, category, category_group, type, hide_from_reports, other_fields
            FROM categories WHERE id = ?"#,
        )
        .bind(id_num)
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

                Ok(Some(_Row {
                    id: r.get::<i64, _>("id") as u64,
                    row: Category {
                        category: r.get("category"),
                        category_group: r
                            .get::<Option<String>, _>("category_group")
                            .unwrap_or_default(),
                        _type: r.get::<Option<String>, _>("type").unwrap_or_default(),
                        hide_from_reports: r
                            .get::<Option<String>, _>("hide_from_reports")
                            .unwrap_or_default(),
                        other_fields,
                    },
                }))
            }
        }
    }

    /// Inserts a new autocat rule into the database. Returns the primary key ID.
    pub(crate) async fn _insert_autocat(&self, autocat: &AutoCat) -> Result<u64> {
        let other_fields_json = if autocat.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&autocat.other_fields)?)
        };

        let result = sqlx::query(
            r#"INSERT INTO autocat (
                category, description, description_contains, account_contains,
                institution_contains, amount_min, amount_max, amount_equals,
                description_equals, description_full, full_description_contains,
                amount_contains, other_fields
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&autocat.category)
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
        .execute(&self.pool)
        .await
        .context("Failed to insert autocat")?;

        Ok(result.last_insert_rowid() as u64)
    }

    /// Updates an existing autocat rule in the database. Returns the primary key ID.
    pub(crate) async fn _update_autocat(&self, autocat: &_Row<AutoCat>) -> Result<u64> {
        let other_fields_json = if autocat.row.other_fields.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&autocat.row.other_fields)?)
        };

        sqlx::query(
            r#"UPDATE autocat SET
                category = ?, description = ?, description_contains = ?, account_contains = ?,
                institution_contains = ?, amount_min = ?, amount_max = ?, amount_equals = ?,
                description_equals = ?, description_full = ?, full_description_contains = ?,
                amount_contains = ?, other_fields = ?
            WHERE id = ?"#,
        )
        .bind(&autocat.row.category)
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
    pub(crate) async fn _get_autocat(&self, id: &str) -> Result<Option<_Row<AutoCat>>> {
        use sqlx::Row;

        let id_num: i64 = id.parse().context("Invalid autocat ID")?;

        let row = sqlx::query(
            r#"SELECT id, category, description, description_contains, account_contains,
                institution_contains, amount_min, amount_max, amount_equals,
                description_equals, description_full, full_description_contains,
                amount_contains, other_fields
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

                let amount_min: Option<f64> = r.get("amount_min");
                let amount_max: Option<f64> = r.get("amount_max");
                let amount_equals: Option<f64> = r.get("amount_equals");

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
                        amount_min: amount_min.and_then(|v| Decimal::from_f64(v).map(Amount::new)),
                        amount_max: amount_max.and_then(|v| Decimal::from_f64(v).map(Amount::new)),
                        amount_equals: amount_equals
                            .and_then(|v| Decimal::from_f64(v).map(Amount::new)),
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
                    },
                }))
            }
        }
    }

    /// Returns a reference to the underlying connection pool.
    #[allow(dead_code)]
    fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Creates the schema_version table and inserts version 0. This establishes the invariant
    /// that schema_version always exists, allowing migration logic to work uniformly.
    async fn bootstrap(&self) -> Result<()> {
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
    async fn schema_version(&self) -> Result<i32> {
        let row: (i32,) = sqlx::query_as("SELECT MAX(version) FROM schema_version")
            .fetch_one(&self.pool)
            .await
            .context("Failed to query schema version")?;
        Ok(row.0)
    }

    /// Runs migrations to bring the database to CURRENT_VERSION.
    async fn migrate(&self) -> Result<()> {
        let current = self.schema_version().await?;
        migrations::run(&self.pool, current, CURRENT_VERSION).await
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
        let transactions = Transactions::new(
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

        let categories = Categories::new(
            vec![
                vec!["Category", "Group", "Type", "Hide From Reports"],
                vec!["Groceries", "Food", "Expense", ""],
            ],
            Vec::<Vec<&str>>::new(),
        )
        .unwrap();

        let auto_cats = AutoCats::new(
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

        db._save_tiller_data(&data).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_tiller_data() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

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

        let data = db._get_tiller_data().await.unwrap();

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

        db._update_transaction(&transaction).await.unwrap();

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

        let transaction = db._get_transaction("txn-001").await.unwrap();

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
        category._type = "Expense".to_string();
        category.hide_from_reports = "".to_string();

        let id = db._insert_category(&category).await.unwrap();

        // Verify by querying directly
        let row: (String, String) =
            sqlx::query_as("SELECT category, category_group FROM categories WHERE id = ?")
                .bind(id as i64)
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

        // Get the inserted ID
        let (id,): (i64,) = sqlx::query_as("SELECT last_insert_rowid()")
            .fetch_one(&db.pool)
            .await
            .unwrap();

        // Update via the method
        let mut category = Category::default();
        category.category = "Updated Groceries".to_string();
        category.category_group = "Updated Food".to_string();
        category._type = "Expense".to_string();
        category.hide_from_reports = "".to_string();

        let row = _Row {
            id: id as u64,
            row: category,
        };

        db._update_category(&row).await.unwrap();

        // Verify the update
        let (name, group): (String, String) =
            sqlx::query_as("SELECT category, category_group FROM categories WHERE id = ?")
                .bind(id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(name, "Updated Groceries");
        assert_eq!(group, "Updated Food");
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

        // Get the inserted ID
        let (id,): (i64,) = sqlx::query_as("SELECT last_insert_rowid()")
            .fetch_one(&db.pool)
            .await
            .unwrap();

        let result = db._get_category(&id.to_string()).await.unwrap();

        assert!(result.is_some());
        let row = result.unwrap();
        assert_eq!(row.id, id as u64);
        assert_eq!(row.row.category, "Groceries");
        assert_eq!(row.row.category_group, "Food");
    }

    #[tokio::test]
    async fn test_insert_autocat() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

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

        // Update via the method
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

        let result = db._get_transaction("non-existent-id").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_category_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.sqlite");
        let db = Db::init(&db_path).await.unwrap();

        let result = db._get_category("99999").await.unwrap();
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
        let retrieved = db._get_transaction("txn-other").await.unwrap().unwrap();
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

        let id = db._insert_category(&category).await.unwrap();

        // Retrieve and verify other_fields was stored
        let retrieved = db._get_category(&id.to_string()).await.unwrap().unwrap();
        assert_eq!(
            retrieved.row.other_fields.get("Extra Field"),
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
}
