//! Database schema migrations.
//!
//! Migration files are stored in this directory with the naming convention:
//! - `migration_NN_up.sql` - Upgrades schema from version `NN-1` to version `NN`
//! - `migration_NN_down.sql` - Downgrades schema from version `NN` to version `NN-1`

use anyhow::{bail, Context};
use sqlx::{Executor, SqlitePool};
use tracing::debug;

use crate::Result;

/// A database migration with up and down SQL.
struct Migration {
    /// The version this migration brings the database to (when going up).
    version: i32,
    /// SQL to execute when upgrading to this version.
    up_sql: &'static str,
    /// SQL to execute when downgrading from this version.
    down_sql: &'static str,
}

/// All available migrations in order.
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    up_sql: include_str!("migration_01_up.sql"),
    down_sql: include_str!("migration_01_down.sql"),
}];

/// Runs migrations to bring the database from `current_version` to `target_version`.
///
/// - If `current_version < target_version`, runs "up" migrations sequentially.
/// - If `current_version > target_version`, runs "down" migrations sequentially.
/// - Each migration is executed within a transaction that includes the schema_version update.
///
/// Validates all required migrations exist before running any of them.
pub(crate) async fn run(pool: &SqlitePool, current_ver: i32, target_ver: i32) -> Result<()> {
    if current_ver == target_ver {
        debug!("Database already at target version {target_ver}, no migrations needed");
        return Ok(());
    }

    // Validate all required migrations exist before running any
    validate_migrations(current_ver, target_ver)?;

    if current_ver < target_ver {
        // Run up migrations
        for version in (current_ver + 1)..=target_ver {
            let migration = MIGRATIONS
                .iter()
                .find(|m| m.version == version)
                .with_context(|| format!("Migration {version} not found"))?;

            debug!("Running migration {version:02} (up)");
            run_single_migration(pool, migration.up_sql, version).await?;
        }
    } else {
        // Run down migrations
        for version in (target_ver + 1..=current_ver).rev() {
            let migration = MIGRATIONS
                .iter()
                .find(|m| m.version == version)
                .with_context(|| format!("Migration {version} not found"))?;

            debug!("Running migration {version:02} (down)");
            run_single_migration(pool, migration.down_sql, version - 1).await?;
        }
    }

    debug!("Migration complete, schema now at version {target_ver}");
    Ok(())
}

/// Executes a single migration's SQL and updates schema_version, all within a transaction.
async fn run_single_migration(pool: &SqlitePool, sql: &str, new_version: i32) -> Result<()> {
    let mut tx = pool
        .begin()
        .await
        .context("Failed to begin migration transaction")?;

    // Execute the migration SQL (supports multiple statements)
    tx.execute(sql)
        .await
        .context("Failed to execute migration SQL")?;

    // Update schema_version
    sqlx::query("DELETE FROM schema_version")
        .execute(&mut *tx)
        .await
        .context("Failed to clear schema_version")?;

    sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
        .bind(new_version)
        .execute(&mut *tx)
        .await
        .context("Failed to update schema_version")?;

    tx.commit()
        .await
        .context("Failed to commit migration transaction")?;

    Ok(())
}

/// Validates that migrations are available for all versions needed to go from
/// `current_version` to `target_version`.
fn validate_migrations(current_version: i32, target_version: i32) -> Result<()> {
    let (start, end) = if current_version < target_version {
        (current_version + 1, target_version)
    } else {
        (target_version + 1, current_version)
    };

    for version in start..=end {
        if !MIGRATIONS.iter().any(|m| m.version == version) {
            bail!(
                "Migration {version} is missing but required to migrate from version {current_version} to {target_version}"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    use tempfile::TempDir;

    /// Helper to create a test database with schema_version bootstrapped at version 0.
    async fn create_test_db() -> Result<(TempDir, SqlitePool)> {
        let temp_dir = TempDir::new().context("Failed to create temp dir")?;
        let db_path = temp_dir.path().join("test.sqlite");

        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
            .context("Failed to parse SQLite connection string")?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .context("Failed to create SQLite database")?;

        // Bootstrap schema_version table
        sqlx::query("CREATE TABLE schema_version (version INTEGER NOT NULL)")
            .execute(&pool)
            .await
            .context("Failed to create schema_version table")?;

        sqlx::query("INSERT INTO schema_version (version) VALUES (0)")
            .execute(&pool)
            .await
            .context("Failed to insert initial schema version")?;

        Ok((temp_dir, pool))
    }

    /// Helper to get current schema version from database.
    async fn get_schema_version(pool: &SqlitePool) -> Result<i32> {
        let row: (i32,) = sqlx::query_as("SELECT MAX(version) FROM schema_version")
            .fetch_one(pool)
            .await
            .context("Failed to query schema version")?;
        Ok(row.0)
    }

    /// Helper to check if a table exists.
    async fn table_exists(pool: &SqlitePool, table_name: &str) -> Result<bool> {
        let row: (i32,) =
            sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?")
                .bind(table_name)
                .fetch_one(pool)
                .await
                .context("Failed to check table existence")?;
        Ok(row.0 > 0)
    }

    #[tokio::test]
    async fn test_migration_up_creates_tables() {
        let (_temp_dir, pool) = create_test_db().await.unwrap();

        // Verify we start at version 0
        assert_eq!(get_schema_version(&pool).await.unwrap(), 0);

        // Run migration from 0 to 1
        run(&pool, 0, 1).await.unwrap();

        // Verify schema version is now 1
        assert_eq!(get_schema_version(&pool).await.unwrap(), 1);

        // Verify all tables were created
        assert!(table_exists(&pool, "transactions").await.unwrap());
        assert!(table_exists(&pool, "categories").await.unwrap());
        assert!(table_exists(&pool, "autocat").await.unwrap());
    }

    #[tokio::test]
    async fn test_migration_down_drops_tables() {
        let (_temp_dir, pool) = create_test_db().await.unwrap();

        // Run migration up first
        run(&pool, 0, 1).await.unwrap();
        assert_eq!(get_schema_version(&pool).await.unwrap(), 1);

        // Run migration down
        run(&pool, 1, 0).await.unwrap();

        // Verify schema version is back to 0
        assert_eq!(get_schema_version(&pool).await.unwrap(), 0);

        // Verify all tables were dropped
        assert!(!table_exists(&pool, "transactions").await.unwrap());
        assert!(!table_exists(&pool, "categories").await.unwrap());
        assert!(!table_exists(&pool, "autocat").await.unwrap());
    }

    #[tokio::test]
    async fn test_migration_no_op_when_already_at_target() {
        let (_temp_dir, pool) = create_test_db().await.unwrap();

        // Run migration to version 1
        run(&pool, 0, 1).await.unwrap();

        // Running again with same version should be a no-op
        run(&pool, 1, 1).await.unwrap();

        // Should still be at version 1
        assert_eq!(get_schema_version(&pool).await.unwrap(), 1);
    }

    #[test]
    fn testvalidate_migrations_succeeds_for_valid_range() {
        // Migration 1 exists, so this should succeed
        assert!(validate_migrations(0, 1).is_ok());
        assert!(validate_migrations(1, 0).is_ok());
    }

    #[test]
    fn testvalidate_migrations_fails_for_missing_migration() {
        // Migration 2 doesn't exist
        assert!(validate_migrations(0, 2).is_err());
        assert!(validate_migrations(1, 3).is_err());
    }
}
