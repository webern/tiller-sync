//! Update command handlers.

use crate::args::{UpdateAutoCatsArgs, UpdateCategoriesArgs, UpdateTransactionsArgs};
use crate::commands::Out;
use crate::db::_Row;
use crate::error::{ErrorType, IntoResult};
use crate::model::{AutoCat, Category, Transaction};
use crate::{Config, Result};

/// Updates one or more transactions by ID with the specified field changes.
///
/// Transactions are updated one-by-one. If an error occurs partway through, some transactions may
/// have been updated while others were not.
///
/// # Arguments
///
/// - `config` - The application configuration containing the database connection.
/// - `args` - The transaction IDs and field updates to apply.
///
/// # Returns
///
/// On success, returns an `Out` containing:
/// - A message indicating how many transactions were updated.
/// - A vector of the updated `Transaction` objects.
///
/// # Errors
///
/// - Returns an error if any specified transaction ID is not found.
/// - Returns an error if a database operation fails.
pub async fn update_transactions(
    config: Config,
    args: UpdateTransactionsArgs,
) -> Result<Out<Vec<Transaction>>> {
    let updated = config
        .db()
        .update_transactions(args)
        .await
        .pub_result(ErrorType::Database)?;
    let count = updated.len();
    let message = format!(
        "Updated {} transaction{}",
        count,
        if count == 1 { "" } else { "s" }
    );
    Ok(Out::new(message, updated))
}

/// Updates one or more categories by name with the specified field changes.
///
/// Categories are updated atomically within a database transaction. If any category is not
/// found, the entire operation is rolled back.
///
/// The category name is the primary key. To rename a category, provide the current name and
/// include the new name in the updates. Due to `ON UPDATE CASCADE` foreign key constraints,
/// renaming a category automatically updates all references in transactions and autocat rules.
///
/// # Arguments
///
/// - `config` - The application configuration containing the database connection.
/// - `args` - The category names and field updates to apply.
///
/// # Returns
///
/// On success, returns an `Out` containing:
/// - A message indicating how many categories were updated.
/// - A vector of the updated `Category` objects.
///
/// # Errors
///
/// - Returns an error if any specified category is not found.
/// - Returns an error if a database operation fails.
pub async fn update_categories(
    config: Config,
    args: UpdateCategoriesArgs,
) -> Result<Out<Vec<Category>>> {
    let updated = config
        .db()
        .update_categories(args)
        .await
        .pub_result(ErrorType::Database)?;
    let count = updated.len();
    let message = format!(
        "Updated {} categor{}",
        count,
        if count == 1 { "y" } else { "ies" }
    );
    Ok(Out::new(message, updated))
}

/// Updates one or more AutoCat rules by ID with the specified field changes.
///
/// AutoCat rules are updated atomically within a database transaction. If any rule is not
/// found, the entire operation is rolled back.
///
/// AutoCat rules have a synthetic auto-increment primary key that is assigned when first synced
/// down or inserted locally.
///
/// # Arguments
///
/// - `config` - The application configuration containing the database connection.
/// - `args` - The AutoCat rule IDs and field updates to apply.
///
/// # Returns
///
/// On success, returns an `Out` containing:
/// - A message indicating how many rules were updated.
/// - A vector of the updated `AutoCat` objects wrapped in `_Row` (includes the ID).
///
/// # Errors
///
/// - Returns an error if any specified AutoCat rule is not found.
/// - Returns an error if a database operation fails.
pub async fn update_autocats(
    config: Config,
    args: UpdateAutoCatsArgs,
) -> Result<Out<Vec<_Row<AutoCat>>>> {
    let updated = config
        .db()
        .update_autocats(args)
        .await
        .pub_result(ErrorType::Database)?;
    let count = updated.len();
    let message = format!(
        "Updated {} AutoCat rule{}",
        count,
        if count == 1 { "" } else { "s" }
    );
    Ok(Out::new(message, updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{UpdateAutoCatsArgs, UpdateCategoriesArgs, UpdateTransactionsArgs};
    use crate::model::{AutoCatUpdates, CategoryUpdates, TransactionUpdates};
    use crate::test::TestEnv;

    #[tokio::test]
    async fn test_update_transactions_success() {
        let env = TestEnv::new().await;
        let txn_id = "test-txn-001";
        env.insert_test_transaction(txn_id).await;

        let updates = TransactionUpdates {
            note: Some("updated note".to_string()),
            ..Default::default()
        };
        let args = UpdateTransactionsArgs::new(vec![txn_id], updates).unwrap();
        let out = update_transactions(env.config(), args).await.unwrap();
        let contains = "Updated 1 transaction";
        assert!(
            out.message().contains(contains),
            "Expected message to contain '{contains}', but message was {}",
            out.message()
        );

        // Verify the update was persisted
        let updated = env
            .config()
            .db()
            ._get_transaction(txn_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.note, "updated note");
    }

    #[tokio::test]
    async fn test_update_transactions_multiple_fields() {
        let env = TestEnv::new().await;
        let txn_id = "test-txn-002";
        env.insert_test_transaction(txn_id).await;

        let updates = TransactionUpdates {
            note: Some("new note".to_string()),
            category: Some("Entertainment".to_string()),
            ..Default::default()
        };
        let args = UpdateTransactionsArgs::new(vec![txn_id], updates).unwrap();
        let out = update_transactions(env.config(), args).await.unwrap();

        let returned = out.structure().unwrap().first().unwrap();
        assert_eq!(returned.note, "new note");
        assert_eq!(returned.category, "Entertainment");
        assert_eq!(returned.account_number, "1234");

        let updated = env
            .config()
            .db()
            ._get_transaction(txn_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.note, "new note");
        assert_eq!(updated.category, "Entertainment");
        assert_eq!(updated.account_number, "1234");
    }

    #[tokio::test]
    async fn test_update_transactions_not_found_error() {
        let env = TestEnv::new().await;
        // Insert test data but query for a different ID
        env.insert_test_transaction("existing-txn").await;

        let updates = TransactionUpdates {
            note: Some("test".to_string()),
            ..Default::default()
        };
        let args = UpdateTransactionsArgs::new(vec!["bad-id"], updates).unwrap();
        let result = update_transactions(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Transaction not found"),
            "Expected 'Transaction not found' but got '{err_msg}'"
        );
    }

    #[tokio::test]
    async fn test_update_transactions_value_with_equals() {
        let env = TestEnv::new().await;
        let txn_id = "test-txn-006";
        env.insert_test_transaction(txn_id).await;

        // Test that values containing '=' are handled correctly
        let updates = TransactionUpdates {
            note: Some("a=b=c".to_string()),
            ..Default::default()
        };
        let args = UpdateTransactionsArgs::new(vec![txn_id], updates).unwrap();
        update_transactions(env.config(), args).await.unwrap();

        let updated = env
            .config()
            .db()
            ._get_transaction(txn_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.note, "a=b=c");
    }

    // === Category update tests ===

    #[tokio::test]
    async fn test_update_categories_success() {
        let env = TestEnv::new().await;
        // insert_test_transaction creates "Food" and "Entertainment" categories
        env.insert_test_transaction("txn-001").await;

        let updates = CategoryUpdates {
            group: Some("Updated Group".to_string()),
            ..Default::default()
        };
        let args = UpdateCategoriesArgs::new(vec!["Food"], updates).unwrap();
        let result = update_categories(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let contains = "Updated 1 category";
        assert!(
            out.message().contains(contains),
            "Expected message to contain '{contains}', but message was {}",
            out.message()
        );

        // Verify the update was persisted
        let updated = env
            .config()
            .db()
            ._get_category("Food")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.category_group, "Updated Group");
    }

    #[tokio::test]
    async fn test_update_categories_multiple_fields() {
        let env = TestEnv::new().await;
        env.insert_test_transaction("txn-001").await;

        let updates = CategoryUpdates {
            group: Some("New Group".to_string()),
            r#type: Some("Income".to_string()),
            hide_from_reports: Some("Hide".to_string()),
            ..Default::default()
        };
        let args = UpdateCategoriesArgs::new(vec!["Food"], updates).unwrap();
        let result = update_categories(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let returned = out.structure().unwrap().first().unwrap();
        assert_eq!(returned.category_group, "New Group");
        assert_eq!(returned.r#type, "Income");
        assert_eq!(returned.hide_from_reports, "Hide");

        let updated = env
            .config()
            .db()
            ._get_category("Food")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.category_group, "New Group");
        assert_eq!(updated.r#type, "Income");
        assert_eq!(updated.hide_from_reports, "Hide");
    }

    #[tokio::test]
    async fn test_update_categories_rename() {
        let env = TestEnv::new().await;
        env.insert_test_transaction("txn-001").await;

        // Rename "Food" to "Groceries"
        let updates = CategoryUpdates {
            category: Some("Groceries".to_string()),
            ..Default::default()
        };
        let args = UpdateCategoriesArgs::new(vec!["Food"], updates).unwrap();
        let result = update_categories(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let contains = "Updated 1 category";
        assert!(
            out.message().contains(contains),
            "Expected message to contain '{contains}', but message was {}",
            out.message()
        );

        // Verify old name no longer exists
        let old = env.config().db()._get_category("Food").await.unwrap();
        assert!(
            old.is_none(),
            "Old category 'Food' should not exist after rename"
        );

        // Verify new name exists
        let new = env.config().db()._get_category("Groceries").await.unwrap();
        assert!(
            new.is_some(),
            "New category 'Groceries' should exist after rename"
        );
    }

    #[tokio::test]
    async fn test_update_categories_not_found_error() {
        let env = TestEnv::new().await;
        env.insert_test_transaction("txn-001").await;

        let updates = CategoryUpdates {
            group: Some("test".to_string()),
            ..Default::default()
        };
        let args = UpdateCategoriesArgs::new(vec!["NonExistent"], updates).unwrap();
        let result = update_categories(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Category not found"));
    }

    // === AutoCat update tests ===

    #[tokio::test]
    async fn test_update_autocats_success() {
        let env = TestEnv::new().await;
        env.insert_test_autocat_data().await;

        let updates = AutoCatUpdates {
            description_contains: Some("updated-pattern".to_string()),
            ..Default::default()
        };
        // AutoCat rules get synthetic IDs starting at 1
        let args = UpdateAutoCatsArgs::new(vec!["1"], updates).unwrap();
        let result = update_autocats(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let contains = "Updated 1 AutoCat rule";
        assert!(
            out.message().contains(contains),
            "Expected message to contain '{contains}', but message was {}",
            out.message()
        );

        // Verify the update was persisted
        let updated = env.config().db()._get_autocat("1").await.unwrap().unwrap();
        assert_eq!(updated.row.description_contains, "updated-pattern");
    }

    #[tokio::test]
    async fn test_update_autocats_multiple_fields() {
        let env = TestEnv::new().await;
        env.insert_test_autocat_data().await;

        let updates = AutoCatUpdates {
            description_contains: Some("new-pattern".to_string()),
            account_contains: Some("checking".to_string()),
            category: Some("Transportation".to_string()),
            ..Default::default()
        };
        let args = UpdateAutoCatsArgs::new(vec!["1"], updates).unwrap();
        let result = update_autocats(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let returned = out.structure().unwrap().first().unwrap();
        assert_eq!(returned.row.description_contains, "new-pattern");
        assert_eq!(returned.row.account_contains, "checking");
        assert_eq!(returned.row.category, "Transportation");

        let updated = env.config().db()._get_autocat("1").await.unwrap().unwrap();
        assert_eq!(updated.row.description_contains, "new-pattern");
        assert_eq!(updated.row.account_contains, "checking");
        assert_eq!(updated.row.category, "Transportation");
    }

    #[tokio::test]
    async fn test_update_autocats_not_found_error() {
        let env = TestEnv::new().await;
        env.insert_test_autocat_data().await;

        let updates = AutoCatUpdates {
            description_contains: Some("test".to_string()),
            ..Default::default()
        };
        let args = UpdateAutoCatsArgs::new(vec!["999"], updates).unwrap();
        let result = update_autocats(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("AutoCat rule not found"));
    }
}
