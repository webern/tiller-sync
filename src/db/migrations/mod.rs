//! Database schema migrations.
//!
//! Migrations can be either SQL scripts or Rust code:
//! - SQL migrations: `migration_NN_up.sql` / `migration_NN_down.sql`
//! - Rust migrations: async functions that manage their own transaction
//!
//! All migrations run within a single transaction that includes the schema_version update.
//! If any part fails, the entire migration is rolled back.

use crate::error::Res;
use crate::model::{Date, DateFromOpt};
use anyhow::{bail, Context};
use sqlx::{Executor, Row, Sqlite, SqlitePool, Transaction};
use std::future::Future;
use std::pin::Pin;
use tracing::{debug, info};

/// Type alias for async Rust migration functions.
///
/// The function receives the pool and the target schema version, and is responsible for:
/// 1. Beginning a transaction
/// 2. Performing the migration work
/// 3. Updating schema_version to `new_version`
/// 4. Committing (or rolling back on error)
type RustMigrationFn = fn(&SqlitePool, i32) -> Pin<Box<dyn Future<Output = Res<()>> + Send + '_>>;

/// Represents a migration action - either SQL or Rust code.
enum MigrationAction {
    /// Execute raw SQL statements (wrapped in a transaction by the runner).
    Sql(&'static str),
    /// Execute a Rust async function that manages its own transaction.
    Rust(RustMigrationFn),
}

/// A database migration with up and down actions.
struct Migration {
    /// The version this migration brings the database to (when going up).
    version: i32,
    /// Action to execute when upgrading to this version.
    up: MigrationAction,
    /// Action to execute when downgrading from this version.
    down: MigrationAction,
}

/// All available migrations in order.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        up: MigrationAction::Sql(include_str!("migration_01_up.sql")),
        down: MigrationAction::Sql(include_str!("migration_01_down.sql")),
    },
    Migration {
        version: 2,
        up: MigrationAction::Rust(migration_02_up),
        down: MigrationAction::Rust(migration_02_down),
    },
    Migration {
        version: 3,
        up: MigrationAction::Sql(include_str!("migration_03_up.sql")),
        down: MigrationAction::Sql(include_str!("migration_03_down.sql")),
    },
];

// ============================================================================
// Migration 02: Convert date columns to ISO format using Rust Date type
// ============================================================================

/// Migration 02 UP: Convert date columns from M/D/YYYY to ISO format.
///
/// Reads each transaction, parses date fields using the Date type, and writes them back.
/// This ensures all date parsing logic is centralized in the Date type.
/// Runs within a single transaction for atomicity - if any conversion fails, all changes roll back.
fn migration_02_up(
    pool: &SqlitePool,
    new_version: i32,
) -> Pin<Box<dyn Future<Output = Res<()>> + Send + '_>> {
    Box::pin(async move {
        let mut tx = pool
            .begin()
            .await
            .context("Failed to begin transaction for migration 02 up")?;

        // Query all transactions with their date fields
        let rows = sqlx::query(
            "SELECT transaction_id, date, month, week, date_added, categorized_date FROM transactions",
        )
        .fetch_all(&mut *tx)
        .await
        .context("Failed to fetch transactions for migration")?;

        let total = rows.len();
        let mut converted = 0;

        for row in rows {
            let id: String = row.try_get("transaction_id")?;
            let date_str: String = row.try_get("date")?;
            let month_str: Option<String> = row.try_get("month")?;
            let week_str: Option<String> = row.try_get("week")?;
            let date_added_str: Option<String> = row.try_get("date_added")?;
            let categorized_date_str: Option<String> = row.try_get("categorized_date")?;

            // Parse and convert date fields using the Date type
            let new_date = Date::parse(&date_str)
                .with_context(|| format!("Failed to parse date '{date_str}' for tx {id}"))?
                .to_string();

            let new_month = month_str.date_from_opt()?;
            let new_week = week_str.date_from_opt()?;
            let new_date_added = date_added_str.date_from_opt()?;
            let new_categorized_date = categorized_date_str.date_from_opt()?;

            // Update the row
            sqlx::query(
                "UPDATE transactions SET date = ?, month = ?, week = ?, date_added = ?, categorized_date = ? WHERE transaction_id = ?",
            )
            .bind(&new_date)
            .bind(&new_month)
            .bind(&new_week)
            .bind(&new_date_added)
            .bind(&new_categorized_date)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Failed to update transaction {id}"))?;

            converted += 1;
        }

        // Update schema version within the same transaction
        update_schema_version(&mut tx, new_version).await?;

        tx.commit()
            .await
            .context("Failed to commit migration 02 up")?;

        info!("Migration 02 up: converted {converted}/{total} transactions to ISO date format");
        Ok(())
    })
}

/// Migration 02 DOWN: Convert date columns back to M/D/YYYY format.
///
/// This is a best-effort reverse migration. Dates are converted back to the original
/// Tiller format (M/D/YYYY for dates, M/D/YYYY H:MM:SS AM/PM for datetimes).
/// Runs within a single transaction for atomicity.
fn migration_02_down(
    pool: &SqlitePool,
    new_version: i32,
) -> Pin<Box<dyn Future<Output = Res<()>> + Send + '_>> {
    Box::pin(async move {
        let mut tx = pool
            .begin()
            .await
            .context("Failed to begin transaction for migration 02 down")?;

        // For the down migration, we use SQL since we're converting FROM a known format
        // YYYY-MM-DD -> M/D/YYYY (strip leading zeros)
        // YYYY-MM-DDTHH:MM:SS+ZZ:ZZ -> M/D/YYYY H:MM:SS AM/PM

        // Convert date column: YYYY-MM-DD -> M/D/YYYY
        sqlx::query(
            r#"
            UPDATE transactions
            SET date = (
                CAST(SUBSTR(date, 6, 2) AS INTEGER)
                || '/'
                || CAST(SUBSTR(date, 9, 2) AS INTEGER)
                || '/'
                || SUBSTR(date, 1, 4)
            )
            WHERE date LIKE '____-__-__'
            "#,
        )
        .execute(&mut *tx)
        .await
        .context("Failed to convert date column")?;

        // Convert date_added column: YYYY-MM-DD -> M/D/YYYY
        sqlx::query(
            r#"
            UPDATE transactions
            SET date_added = (
                CAST(SUBSTR(date_added, 6, 2) AS INTEGER)
                || '/'
                || CAST(SUBSTR(date_added, 9, 2) AS INTEGER)
                || '/'
                || SUBSTR(date_added, 1, 4)
            )
            WHERE date_added IS NOT NULL
              AND date_added != ''
              AND date_added LIKE '____-__-__'
            "#,
        )
        .execute(&mut *tx)
        .await
        .context("Failed to convert date_added column")?;

        // Convert categorized_date: YYYY-MM-DDTHH:MM:SS... -> M/D/YYYY H:MM:SS AM/PM
        // This is complex due to 24h->12h conversion, so we handle common cases
        let rows = sqlx::query(
            "SELECT transaction_id, categorized_date FROM transactions
             WHERE categorized_date IS NOT NULL
               AND categorized_date != ''
               AND categorized_date LIKE '____-__-__T%'",
        )
        .fetch_all(&mut *tx)
        .await
        .context("Failed to fetch categorized_date values")?;

        for row in rows {
            let id: String = row.get("transaction_id");
            let dt_str: String = row.get("categorized_date");

            // Parse ISO datetime and convert to M/D/YYYY H:MM:SS AM/PM
            let new_value = convert_iso_to_us_datetime(&dt_str)?;

            sqlx::query("UPDATE transactions SET categorized_date = ? WHERE transaction_id = ?")
                .bind(&new_value)
                .bind(&id)
                .execute(&mut *tx)
                .await
                .with_context(|| format!("Failed to update categorized_date for tx {id}"))?;
        }

        // Update schema version within the same transaction
        update_schema_version(&mut tx, new_version).await?;

        tx.commit()
            .await
            .context("Failed to commit migration 02 down")?;

        info!("Migration 02 down: reverted transactions to M/D/YYYY format");
        Ok(())
    })
}

/// Convert ISO datetime (YYYY-MM-DDTHH:MM:SS with optional timezone) to US format.
fn convert_iso_to_us_datetime(s: &str) -> Res<String> {
    // Extract date part (first 10 chars): YYYY-MM-DD
    let date_part = &s[0..10];
    let year = &date_part[0..4];
    let month: i32 = date_part[5..7].parse().context("Invalid month")?;
    let day: i32 = date_part[8..10].parse().context("Invalid day")?;

    // Extract time part (chars 11-19): THH:MM:SS
    let time_part = &s[11..19];
    let hour24: i32 = time_part[0..2].parse().context("Invalid hour")?;
    let minute = &time_part[3..5];
    let second = &time_part[6..8];

    // Convert 24h to 12h
    let (hour12, ampm) = match hour24 {
        0 => (12, "AM"),
        1..=11 => (hour24, "AM"),
        12 => (12, "PM"),
        13..=23 => (hour24 - 12, "PM"),
        _ => bail!("Invalid hour: {hour24}"),
    };

    Ok(format!(
        "{month}/{day}/{year} {hour12}:{minute}:{second} {ampm}"
    ))
}

// ============================================================================
// Migration runner
// ============================================================================

/// Runs migrations to bring the database from `current_version` to `target_version`.
///
/// - If `current_version < target_version`, runs "up" migrations sequentially.
/// - If `current_version > target_version`, runs "down" migrations sequentially.
/// - SQL migrations run within a transaction; Rust migrations manage their own transactions.
///
/// Validates all required migrations exist before running any of them.
pub(crate) async fn run(pool: &SqlitePool, current_ver: i32, target_ver: i32) -> Res<()> {
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
            run_migration_action(pool, &migration.up, version).await?;
        }
    } else {
        // Run down migrations
        for version in (target_ver + 1..=current_ver).rev() {
            let migration = MIGRATIONS
                .iter()
                .find(|m| m.version == version)
                .with_context(|| format!("Migration {version} not found"))?;

            debug!("Running migration {version:02} (down)");
            run_migration_action(pool, &migration.down, version - 1).await?;
        }
    }

    debug!("Migration complete, schema now at version {target_ver}");
    Ok(())
}

/// Executes a single migration action and updates schema_version.
///
/// - SQL migrations: wrapped in a transaction by this function
/// - Rust migrations: manage their own transaction (including schema_version update)
async fn run_migration_action(
    pool: &SqlitePool,
    action: &MigrationAction,
    new_version: i32,
) -> Res<()> {
    match action {
        MigrationAction::Sql(sql) => {
            // SQL migrations are wrapped in a transaction here
            let mut tx = pool
                .begin()
                .await
                .context("Failed to begin migration transaction")?;

            tx.execute(*sql)
                .await
                .context("Failed to execute migration SQL")?;

            // Update schema version within the same transaction
            update_schema_version(&mut tx, new_version).await?;

            tx.commit()
                .await
                .context("Failed to commit migration transaction")?;
        }
        MigrationAction::Rust(func) => {
            // Rust migrations manage their own transaction (including schema_version update)
            func(pool, new_version).await?;
        }
    }

    Ok(())
}

/// Helper to update schema_version within a transaction.
async fn update_schema_version(tx: &mut Transaction<'_, Sqlite>, new_version: i32) -> Res<()> {
    sqlx::query("DELETE FROM schema_version")
        .execute(&mut **tx)
        .await
        .context("Failed to clear schema_version")?;

    sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
        .bind(new_version)
        .execute(&mut **tx)
        .await
        .context("Failed to update schema_version")?;

    Ok(())
}

/// Validates that migrations are available for all versions needed to go from
/// `current_version` to `target_version`.
fn validate_migrations(current_version: i32, target_version: i32) -> Res<()> {
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
    async fn create_test_db() -> Res<(TempDir, SqlitePool)> {
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
    async fn get_schema_version(pool: &SqlitePool) -> Res<i32> {
        let row: (i32,) = sqlx::query_as("SELECT MAX(version) FROM schema_version")
            .fetch_one(pool)
            .await
            .context("Failed to query schema version")?;
        Ok(row.0)
    }

    /// Helper to check if a table exists.
    async fn table_exists(pool: &SqlitePool, table_name: &str) -> Res<bool> {
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
        // Every migration up to CURRENT_VERSION exists, in both directions.
        for version in 1..=crate::db::CURRENT_VERSION {
            assert!(validate_migrations(0, version).is_ok());
            assert!(validate_migrations(version, 0).is_ok());
        }
    }

    #[test]
    fn testvalidate_migrations_fails_for_missing_migration() {
        // There is no migration beyond CURRENT_VERSION.
        let beyond = crate::db::CURRENT_VERSION + 1;
        assert!(validate_migrations(0, beyond).is_err());
        assert!(validate_migrations(beyond, beyond + 1).is_err());
    }

    #[tokio::test]
    async fn test_migration_02_converts_dates_to_iso_format() {
        let (_temp_dir, pool) = create_test_db().await.unwrap();

        // Run migration to version 1 (creates tables)
        run(&pool, 0, 1).await.unwrap();

        // Insert a category first (required for foreign key)
        sqlx::query("INSERT INTO categories (category) VALUES ('Food')")
            .execute(&pool)
            .await
            .unwrap();

        // Insert test transactions with old date formats
        sqlx::query(
            r#"
            INSERT INTO transactions (
                transaction_id, date, description, amount, account, account_number,
                institution, account_id, date_added, categorized_date
            ) VALUES
            -- Basic M/D/YYYY format
            ('tx1', '1/23/2025', 'Test 1', -50.00, 'Checking', 'xxxx1234',
             'Bank', 'acc1', '12/31/2024', '1/15/2025 10:30:45 AM'),
            -- Two-digit month and day
            ('tx2', '12/5/2024', 'Test 2', -25.00, 'Checking', 'xxxx1234',
             'Bank', 'acc2', '11/1/2024', '11/20/2024 2:05:30 PM'),
            -- Midnight and noon edge cases
            ('tx3', '7/4/2025', 'Test 3', -100.00, 'Savings', 'xxxx5678',
             'Bank', 'acc3', NULL, '7/4/2025 12:00:00 AM'),
            -- PM noon case
            ('tx4', '6/15/2025', 'Test 4', -75.00, 'Savings', 'xxxx5678',
             'Bank', 'acc4', '6/1/2025', '6/15/2025 12:00:00 PM'),
            -- NULL categorized_date
            ('tx5', '3/10/2025', 'Test 5', -30.00, 'Checking', 'xxxx1234',
             'Bank', 'acc5', '3/1/2025', NULL)
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Run migration 02 (Rust-based)
        run(&pool, 1, 2).await.unwrap();
        assert_eq!(get_schema_version(&pool).await.unwrap(), 2);

        // Verify date conversions
        let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT transaction_id, date, date_added, categorized_date FROM transactions ORDER BY transaction_id"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        // tx1: 1/23/2025 -> 2025-01-23, 12/31/2024 -> 2024-12-31, 1/15/2025 10:30:45 AM -> 2025-01-15T10:30:45
        assert_eq!(rows[0].0, "tx1");
        assert_eq!(rows[0].1, "2025-01-23");
        assert_eq!(rows[0].2, Some("2024-12-31".to_string()));
        assert_eq!(rows[0].3, Some("2025-01-15T10:30:45".to_string()));

        // tx2: 12/5/2024 -> 2024-12-05, 11/1/2024 -> 2024-11-01, 11/20/2024 2:05:30 PM -> 2024-11-20T14:05:30
        assert_eq!(rows[1].0, "tx2");
        assert_eq!(rows[1].1, "2024-12-05");
        assert_eq!(rows[1].2, Some("2024-11-01".to_string()));
        assert_eq!(rows[1].3, Some("2024-11-20T14:05:30".to_string()));

        // tx3: 7/4/2025 -> 2025-07-04, NULL date_added, 7/4/2025 12:00:00 AM -> 2025-07-04T00:00:00
        assert_eq!(rows[2].0, "tx3");
        assert_eq!(rows[2].1, "2025-07-04");
        assert_eq!(rows[2].2, None);
        assert_eq!(rows[2].3, Some("2025-07-04T00:00:00".to_string()));

        // tx4: 6/15/2025 -> 2025-06-15, 6/1/2025 -> 2025-06-01, 6/15/2025 12:00:00 PM -> 2025-06-15T12:00:00
        assert_eq!(rows[3].0, "tx4");
        assert_eq!(rows[3].1, "2025-06-15");
        assert_eq!(rows[3].2, Some("2025-06-01".to_string()));
        assert_eq!(rows[3].3, Some("2025-06-15T12:00:00".to_string()));

        // tx5: 3/10/2025 -> 2025-03-10, 3/1/2025 -> 2025-03-01, NULL categorized_date
        assert_eq!(rows[4].0, "tx5");
        assert_eq!(rows[4].1, "2025-03-10");
        assert_eq!(rows[4].2, Some("2025-03-01".to_string()));
        assert_eq!(rows[4].3, None);
    }

    #[tokio::test]
    async fn test_migration_02_down_reverts_to_mdy_format() {
        let (_temp_dir, pool) = create_test_db().await.unwrap();

        // Run migrations up to version 2
        run(&pool, 0, 1).await.unwrap();

        // Insert a category first
        sqlx::query("INSERT INTO categories (category) VALUES ('Food')")
            .execute(&pool)
            .await
            .unwrap();

        // Insert test data already in ISO format (simulating post-migration state)
        sqlx::query(
            r#"
            INSERT INTO transactions (
                transaction_id, date, description, amount, account, account_number,
                institution, account_id, date_added, categorized_date
            ) VALUES
            ('tx1', '2025-01-23', 'Test 1', -50.00, 'Checking', 'xxxx1234',
             'Bank', 'acc1', '2024-12-31', '2025-01-15T10:30:45'),
            ('tx2', '2024-12-05', 'Test 2', -25.00, 'Checking', 'xxxx1234',
             'Bank', 'acc2', '2024-11-01', '2024-11-20T14:05:30'),
            ('tx3', '2025-07-04', 'Test 3', -100.00, 'Savings', 'xxxx5678',
             'Bank', 'acc3', NULL, '2025-07-04T00:00:00')
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        // Manually set version to 2 since we inserted ISO data directly
        sqlx::query("DELETE FROM schema_version")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO schema_version (version) VALUES (2)")
            .execute(&pool)
            .await
            .unwrap();

        // Run migration down from 2 to 1 (Rust-based)
        run(&pool, 2, 1).await.unwrap();
        assert_eq!(get_schema_version(&pool).await.unwrap(), 1);

        // Verify date conversions back to M/D/YYYY format
        let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT transaction_id, date, date_added, categorized_date FROM transactions ORDER BY transaction_id"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        // tx1: 2025-01-23 -> 1/23/2025, 2024-12-31 -> 12/31/2024, 2025-01-15T10:30:45 -> 1/15/2025 10:30:45 AM
        assert_eq!(rows[0].0, "tx1");
        assert_eq!(rows[0].1, "1/23/2025");
        assert_eq!(rows[0].2, Some("12/31/2024".to_string()));
        assert_eq!(rows[0].3, Some("1/15/2025 10:30:45 AM".to_string()));

        // tx2: 2024-12-05 -> 12/5/2024, 2024-11-01 -> 11/1/2024, 2024-11-20T14:05:30 -> 11/20/2024 2:05:30 PM
        assert_eq!(rows[1].0, "tx2");
        assert_eq!(rows[1].1, "12/5/2024");
        assert_eq!(rows[1].2, Some("11/1/2024".to_string()));
        assert_eq!(rows[1].3, Some("11/20/2024 2:05:30 PM".to_string()));

        // tx3: 2025-07-04 -> 7/4/2025, NULL, 2025-07-04T00:00:00 -> 7/4/2025 12:00:00 AM
        assert_eq!(rows[2].0, "tx3");
        assert_eq!(rows[2].1, "7/4/2025");
        assert_eq!(rows[2].2, None);
        assert_eq!(rows[2].3, Some("7/4/2025 12:00:00 AM".to_string()));
    }
}
