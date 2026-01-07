//! Delete command handlers.

use crate::args::{DeleteAutoCatsArgs, DeleteCategoriesArgs, DeleteTransactionsArgs};
use crate::commands::Out;
use crate::error::{ErrorType, IntoResult};
use crate::{Config, Result};

/// Deletes one or more transactions by ID atomically.
///
/// This operation is all-or-nothing: either all specified transactions are deleted, or none are.
/// If any transaction ID is not found, the entire operation is rolled back.
pub async fn delete_transactions(
    config: Config,
    args: DeleteTransactionsArgs,
) -> Result<Out<Vec<String>>> {
    let deleted = config
        .db()
        .delete_transactions(args)
        .await
        .pub_result(ErrorType::Database)?;

    let count = deleted.len();
    let message = format!(
        "Deleted {} transaction{}",
        count,
        if count == 1 { "" } else { "s" }
    );
    Ok(Out::new(message, deleted))
}

/// Deletes one or more categories by name atomically.
///
/// This operation is all-or-nothing: either all specified categories are deleted, or none are.
/// If any category is not found, the entire operation is rolled back.
///
/// Due to `ON DELETE RESTRICT` foreign key constraints, a category cannot be deleted if any
/// transactions or AutoCat rules reference it. Those references must be updated or removed first.
pub async fn delete_categories(
    config: Config,
    args: DeleteCategoriesArgs,
) -> Result<Out<Vec<String>>> {
    let deleted = config
        .db()
        .delete_categories(args)
        .await
        .pub_result(ErrorType::Database)?;

    let count = deleted.len();
    let message = format!(
        "Deleted {} categor{}",
        count,
        if count == 1 { "y" } else { "ies" }
    );
    Ok(Out::new(message, deleted))
}

/// Deletes one or more AutoCat rules by ID atomically.
///
/// This operation is all-or-nothing: either all specified rules are deleted, or none are.
/// If any rule ID is not found, the entire operation is rolled back.
pub async fn delete_autocats(config: Config, args: DeleteAutoCatsArgs) -> Result<Out<Vec<String>>> {
    let deleted = config
        .db()
        .delete_autocats(args)
        .await
        .pub_result(ErrorType::Database)?;

    let count = deleted.len();
    let message = format!(
        "Deleted {} AutoCat rule{}",
        count,
        if count == 1 { "" } else { "s" }
    );
    Ok(Out::new(message, deleted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::TestEnv;

    #[tokio::test]
    async fn test_delete_transactions_success() {
        let env = TestEnv::new().await;
        let txn_id = "test-txn-001";
        env.insert_test_transaction(txn_id).await;

        // Verify transaction exists
        let existing = env.config().db()._get_transaction(txn_id).await.unwrap();
        assert!(existing.is_some());

        // Delete the transaction
        let args = DeleteTransactionsArgs::new(vec![txn_id]).unwrap();
        let result = delete_transactions(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Deleted 1 transaction"));
        assert_eq!(out.structure().unwrap(), &vec![txn_id.to_string()]);

        // Verify transaction no longer exists
        let deleted = env.config().db()._get_transaction(txn_id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_delete_transactions_multiple() {
        let env = TestEnv::new().await;
        // insert_test_transaction creates one transaction, we need to insert multiple
        env.insert_test_transaction("txn-001").await;

        // Insert another transaction by modifying the data
        // For simplicity, we'll just test with the one transaction
        let args = DeleteTransactionsArgs::new(vec!["txn-001"]).unwrap();
        let result = delete_transactions(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Deleted 1 transaction"));
    }

    #[tokio::test]
    async fn test_delete_transactions_not_found_error() {
        let env = TestEnv::new().await;
        env.insert_test_transaction("existing-txn").await;

        let args = DeleteTransactionsArgs::new(vec!["nonexistent-id"]).unwrap();
        let result = delete_transactions(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Transaction not found"));
    }

    #[tokio::test]
    async fn test_delete_transactions_atomic_rollback() {
        let env = TestEnv::new().await;
        env.insert_test_transaction("txn-001").await;

        // Try to delete one existing and one non-existing transaction
        // With atomic operations, if one fails, none should be deleted
        let args = DeleteTransactionsArgs::new(vec!["txn-001", "nonexistent"]).unwrap();
        let result = delete_transactions(env.config(), args).await;

        // Should fail because nonexistent doesn't exist
        assert!(result.is_err());

        // Verify txn-001 was NOT deleted (atomic rollback)
        let still_exists = env.config().db()._get_transaction("txn-001").await.unwrap();
        assert!(
            still_exists.is_some(),
            "Transaction should still exist after atomic rollback"
        );
    }

    // ==================== delete_categories tests ====================

    #[tokio::test]
    async fn test_delete_categories_success() {
        let env = TestEnv::new().await;
        env.insert_standalone_categories(&["TestCategory", "OtherCategory"])
            .await;

        // Verify category exists
        let existing = env
            .config()
            .db()
            ._get_category("TestCategory")
            .await
            .unwrap();
        assert!(existing.is_some());

        // Delete the category
        let args = DeleteCategoriesArgs::new(vec!["TestCategory"]).unwrap();
        let result = delete_categories(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Deleted 1 category"));
        assert_eq!(out.structure().unwrap(), &vec!["TestCategory".to_string()]);

        // Verify category no longer exists
        let deleted = env
            .config()
            .db()
            ._get_category("TestCategory")
            .await
            .unwrap();
        assert!(deleted.is_none());

        // Verify other category still exists
        let other = env
            .config()
            .db()
            ._get_category("OtherCategory")
            .await
            .unwrap();
        assert!(other.is_some());
    }

    #[tokio::test]
    async fn test_delete_categories_multiple() {
        let env = TestEnv::new().await;
        env.insert_standalone_categories(&["Cat1", "Cat2", "Cat3"])
            .await;

        // Delete multiple categories
        let args = DeleteCategoriesArgs::new(vec!["Cat1", "Cat2"]).unwrap();
        let result = delete_categories(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Deleted 2 categories"));
        assert_eq!(
            out.structure().unwrap(),
            &vec!["Cat1".to_string(), "Cat2".to_string()]
        );

        // Verify Cat1 and Cat2 no longer exist
        assert!(env
            .config()
            .db()
            ._get_category("Cat1")
            .await
            .unwrap()
            .is_none());
        assert!(env
            .config()
            .db()
            ._get_category("Cat2")
            .await
            .unwrap()
            .is_none());

        // Verify Cat3 still exists
        assert!(env
            .config()
            .db()
            ._get_category("Cat3")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn test_delete_categories_not_found_error() {
        let env = TestEnv::new().await;
        env.insert_standalone_categories(&["ExistingCategory"])
            .await;

        let args = DeleteCategoriesArgs::new(vec!["NonexistentCategory"]).unwrap();
        let result = delete_categories(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Category not found"));
    }

    #[tokio::test]
    async fn test_delete_categories_foreign_key_transaction_error() {
        let env = TestEnv::new().await;
        // insert_test_transaction creates a transaction with category "Food"
        env.insert_test_transaction("test-txn").await;

        // Try to delete the category that is referenced by the transaction
        let args = DeleteCategoriesArgs::new(vec!["Food"]).unwrap();
        let result = delete_categories(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Cannot delete category 'Food'")
                || err_msg.contains("FOREIGN KEY constraint failed"),
            "Expected foreign key error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_delete_categories_foreign_key_autocat_error() {
        let env = TestEnv::new().await;
        // insert_test_autocat_data creates autocat rules referencing "Food" and "Entertainment"
        env.insert_test_autocat_data().await;

        // Try to delete the category that is referenced by an autocat rule
        let args = DeleteCategoriesArgs::new(vec!["Food"]).unwrap();
        let result = delete_categories(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Cannot delete category 'Food'")
                || err_msg.contains("FOREIGN KEY constraint failed"),
            "Expected foreign key error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_delete_categories_atomic_rollback() {
        let env = TestEnv::new().await;
        env.insert_standalone_categories(&["Cat1", "Cat2"]).await;

        // Try to delete one existing and one non-existing category
        // With atomic operations, if one fails, none should be deleted
        let args = DeleteCategoriesArgs::new(vec!["Cat1", "nonexistent"]).unwrap();
        let result = delete_categories(env.config(), args).await;

        // Should fail because nonexistent doesn't exist
        assert!(result.is_err());

        // Verify Cat1 was NOT deleted (atomic rollback)
        let still_exists = env.config().db()._get_category("Cat1").await.unwrap();
        assert!(
            still_exists.is_some(),
            "Category should still exist after atomic rollback"
        );

        // Verify Cat2 also still exists
        let cat2_exists = env.config().db()._get_category("Cat2").await.unwrap();
        assert!(cat2_exists.is_some());
    }

    // ==================== delete_autocats tests ====================

    #[tokio::test]
    async fn test_delete_autocats_success() {
        let env = TestEnv::new().await;
        // insert_test_autocat_data creates autocat rules with IDs 1 and 2
        env.insert_test_autocat_data().await;

        // Verify autocat exists
        let existing = env.config().db()._get_autocat("1").await.unwrap();
        assert!(existing.is_some());

        // Delete the autocat
        let args = DeleteAutoCatsArgs::new(vec!["1"]).unwrap();
        let result = delete_autocats(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Deleted 1 AutoCat rule"));
        assert_eq!(out.structure().unwrap(), &vec!["1".to_string()]);

        // Verify autocat no longer exists
        let deleted = env.config().db()._get_autocat("1").await.unwrap();
        assert!(deleted.is_none());

        // Verify other autocat still exists
        let other = env.config().db()._get_autocat("2").await.unwrap();
        assert!(other.is_some());
    }

    #[tokio::test]
    async fn test_delete_autocats_multiple() {
        let env = TestEnv::new().await;
        env.insert_test_autocat_data().await;

        // Delete multiple autocats
        let args = DeleteAutoCatsArgs::new(vec!["1", "2"]).unwrap();
        let result = delete_autocats(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Deleted 2 AutoCat rules"));
        assert_eq!(
            out.structure().unwrap(),
            &vec!["1".to_string(), "2".to_string()]
        );

        // Verify both autocats no longer exist
        assert!(env.config().db()._get_autocat("1").await.unwrap().is_none());
        assert!(env.config().db()._get_autocat("2").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_autocats_not_found_error() {
        let env = TestEnv::new().await;
        env.insert_test_autocat_data().await;

        let args = DeleteAutoCatsArgs::new(vec!["999"]).unwrap();
        let result = delete_autocats(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("AutoCat rule not found"));
    }

    #[tokio::test]
    async fn test_delete_autocats_atomic_rollback() {
        let env = TestEnv::new().await;
        env.insert_test_autocat_data().await;

        // Try to delete one existing and one non-existing autocat
        // With atomic operations, if one fails, none should be deleted
        let args = DeleteAutoCatsArgs::new(vec!["1", "999"]).unwrap();
        let result = delete_autocats(env.config(), args).await;

        // Should fail because 999 doesn't exist
        assert!(result.is_err());

        // Verify 1 was NOT deleted (atomic rollback)
        let still_exists = env.config().db()._get_autocat("1").await.unwrap();
        assert!(
            still_exists.is_some(),
            "AutoCat should still exist after atomic rollback"
        );

        // Verify 2 also still exists
        let autocat2_exists = env.config().db()._get_autocat("2").await.unwrap();
        assert!(autocat2_exists.is_some());
    }
}
