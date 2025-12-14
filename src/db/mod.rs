//! This module is responsible for reading, writing and managing the SQLite database. The internal
//! details of SQLite interaction are hidden while broader functions are exposed.

mod migrations;

use crate::Result;

/// The target schema version for the database. This equals the highest migration number available.
/// When `migration_05_up.sql` is the highest numbered migration, this should be `5`.
pub(crate) const CURRENT_VERSION: i32 = 1;
use anyhow::{bail, Context};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

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
            .max_connections(5)
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
            .max_connections(5)
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
}
