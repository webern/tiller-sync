//! Query commands for executing SQL and retrieving schema information.
//!
//! This module provides:
//! - `query`: Execute arbitrary read-only SQL queries
//! - `schema`: Retrieve database schema information

use crate::args::{QueryArgs, SchemaArgs};
use crate::commands::Out;
use crate::error::{ErrorType, IntoResult};
use crate::Config;
use crate::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};

// =============================================================================
// Rows type for query results
// =============================================================================

/// Query result rows in the requested output format.
#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Rows {
    /// JSON array of objects where each row is a self-describing object with column names as keys.
    Json(serde_json::Value),
    /// Markdown table as a single formatted string.
    Table(String),
    /// CSV data as a properly escaped string.
    Csv(String),
}

impl Debug for Rows {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Rows::Json(v) => write!(f, "Rows::Json({:?})", v),
            Rows::Table(s) => write!(f, "Rows::Table({} chars)", s.len()),
            Rows::Csv(s) => write!(f, "Rows::Csv({} chars)", s.len()),
        }
    }
}

impl Display for Rows {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Rows::Json(v) => {
                if let Ok(s) = serde_json::to_string_pretty(v) {
                    write!(f, "{}", s)
                } else {
                    write!(f, "{:?}", v)
                }
            }
            Rows::Table(s) => write!(f, "{}", s),
            Rows::Csv(s) => write!(f, "{}", s),
        }
    }
}

// =============================================================================
// Schema types for schema command
// =============================================================================

/// Database schema information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Schema {
    /// List of tables in the database.
    pub tables: Vec<TableInfo>,
}

/// Information about a database table.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableInfo {
    /// Table name.
    pub name: String,
    /// Number of rows in the table.
    pub row_count: u64,
    /// Columns in the table.
    pub columns: Vec<ColumnInfo>,
    /// Indexes on the table.
    pub indexes: Vec<IndexInfo>,
    /// Foreign key constraints.
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

/// Information about a table column.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ColumnInfo {
    /// Column name.
    pub name: String,
    /// SQLite data type.
    pub data_type: String,
    /// Whether the column allows NULL values.
    pub nullable: bool,
    /// Whether the column is part of the primary key.
    pub primary_key: bool,
    /// Description of the column from model doc comments (if available).
    pub description: Option<String>,
}

/// Information about a table index.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexInfo {
    /// Index name.
    pub name: String,
    /// Columns included in the index.
    pub columns: Vec<String>,
    /// Whether the index enforces uniqueness.
    pub unique: bool,
}

/// Information about a foreign key constraint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ForeignKeyInfo {
    /// Columns in this table that are part of the foreign key.
    pub columns: Vec<String>,
    /// Table that this foreign key references.
    pub references_table: String,
    /// Columns in the referenced table.
    pub references_columns: Vec<String>,
}

// =============================================================================
// Command implementations
// =============================================================================

/// Execute a read-only SQL query against the local SQLite database.
///
/// The query interface enforces read-only access using a separate SQLite connection opened with
/// `?mode=ro`. Any write attempt (INSERT, UPDATE, DELETE) will be rejected by SQLite.
pub async fn query(config: Config, args: QueryArgs) -> Result<Out<Rows>> {
    config
        .db()
        .execute_query(args)
        .await
        .pub_result(ErrorType::Database)
}

/// Retrieve database schema information.
///
/// Returns tables, columns, types, indexes, foreign keys, column descriptions, and row counts.
pub async fn schema(config: Config, args: SchemaArgs) -> Result<Out<Schema>> {
    config
        .db()
        .get_schema(args)
        .await
        .pub_result(ErrorType::Database)
}
