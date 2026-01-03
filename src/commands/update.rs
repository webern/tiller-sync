//! Update command handlers.

use crate::args::UpdateTransactionsArgs;
use crate::commands::Out;
use crate::error::{ErrorType, IntoResult};
use crate::model::{Transaction, TransactionColumn};
use crate::{Config, Result};
use anyhow::{anyhow, Context};
use std::collections::HashMap;

pub type Updates = HashMap<TransactionColumn, String>;

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
    let mut updated = Vec::new();

    for id in args.ids() {
        // Fetch existing transaction
        let mut transaction = config
            .db()
            .get_transaction(id)
            .await
            .pub_result(ErrorType::Database)?
            .ok_or_else(|| anyhow!("Transaction not found: {}", id))
            .pub_result(ErrorType::Internal)?;

        transaction.merge_updates(args.updates().clone());

        // Save updated transaction
        config
            .db()
            .update_transaction(&transaction)
            .await
            .pub_result(ErrorType::Database)?;

        updated.push(
            config
                .db()
                .get_transaction(id)
                .await
                .pub_result(ErrorType::Database)?
                .with_context(|| format!("Transaction {id} not found after updating it"))
                .pub_result(ErrorType::Database)?,
        );
    }

    let message = format!("Updated {} transactions", updated.len());
    Ok(Out::new(message, updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::UpdateTransactionsArgs;
    use crate::model::TransactionUpdates;
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
        let result = update_transactions(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
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
            .get_transaction(txn_id)
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
        let result = update_transactions(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let returned = out.structure().unwrap().first().unwrap();
        assert_eq!(returned.note, "new note");
        assert_eq!(returned.category, "Entertainment");
        assert_eq!(returned.account_number, "1234");

        let updated = env
            .config()
            .db()
            .get_transaction(txn_id)
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
        assert!(err_msg.contains("Transaction not found"));
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
        let result = update_transactions(env.config(), args).await;

        assert!(result.is_ok());

        let updated = env
            .config()
            .db()
            .get_transaction(txn_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.note, "a=b=c");
    }
}
