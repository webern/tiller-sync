//! Insert command handlers.

use crate::args::{InsertAutoCatArgs, InsertCategoryArgs, InsertTransactionArgs};
use crate::commands::Out;
use crate::error::{ErrorType, IntoResult};
use crate::model::{AutoCat, Category, Transaction};
use crate::utils::generate_transaction_id;
use crate::{Config, Result};

/// Inserts a new transaction into the local SQLite database.
///
/// A unique transaction ID is automatically generated with a `user-` prefix to distinguish it
/// from Tiller-created transactions. The generated ID is returned on success.
///
/// # Arguments
///
/// - `config` - The application configuration containing the database connection.
/// - `args` - The transaction data to insert. `date` and `amount` are required; all other fields
///   are optional.
///
/// # Returns
///
/// On success, returns an `Out` containing:
/// - A message indicating the transaction was inserted.
/// - The generated transaction ID.
///
/// # Errors
///
/// - Returns an error if a database operation fails.
/// - Returns an error if the specified category does not exist (foreign key constraint).
pub async fn insert_transaction(
    config: Config,
    args: InsertTransactionArgs,
) -> Result<Out<String>> {
    // Generate a unique transaction ID
    let transaction_id = generate_transaction_id();

    // Build the Transaction object from args
    let transaction = Transaction {
        transaction_id: transaction_id.clone(),
        date: args.date,
        amount: args.amount,
        description: args.description.unwrap_or_default(),
        account: args.account.unwrap_or_default(),
        account_number: args.account_number.unwrap_or_default(),
        institution: args.institution.unwrap_or_default(),
        month: args.month.unwrap_or_default(),
        week: args.week.unwrap_or_default(),
        full_description: args.full_description.unwrap_or_default(),
        account_id: args.account_id.unwrap_or_default(),
        check_number: args.check_number.unwrap_or_default(),
        date_added: args.date_added.unwrap_or_default(),
        merchant_name: args.merchant_name.unwrap_or_default(),
        category_hint: args.category_hint.unwrap_or_default(),
        category: args.category.clone().unwrap_or_default(),
        note: args.note.unwrap_or_default(),
        tags: args.tags.unwrap_or_default(),
        categorized_date: args.categorized_date.unwrap_or_default(),
        statement: args.statement.unwrap_or_default(),
        metadata: args.metadata.unwrap_or_default(),
        no_name: String::new(),
        other_fields: args.other_fields,
        original_order: None, // Locally-added rows have no original order
    };

    // Insert into database
    config
        .db()
        .insert_transaction(&transaction)
        .await
        .map_err(|e| {
            // Check if this is a foreign key constraint error
            let err_str = e.to_string();
            if err_str.contains("FOREIGN KEY constraint failed") {
                anyhow::anyhow!(
                    "Cannot insert transaction: category '{}' does not exist. \
                     Create the category first or leave the category field empty.",
                    args.category.as_deref().unwrap_or("")
                )
            } else {
                e
            }
        })
        .pub_result(ErrorType::Database)?;

    let message = format!("Inserted transaction with ID: {}", transaction_id);
    Ok(Out::new(message, transaction_id))
}

/// Inserts a new category into the local SQLite database.
///
/// The category name is the primary key and must be unique. The name is returned on success.
///
/// # Arguments
///
/// - `config` - The application configuration containing the database connection.
/// - `args` - The category data to insert. `name` is required; all other fields are optional.
///
/// # Returns
///
/// On success, returns an `Out` containing:
/// - A message indicating the category was inserted.
/// - The category name (primary key).
///
/// # Errors
///
/// - Returns an error if a category with the same name already exists.
/// - Returns an error if a database operation fails.
pub async fn insert_category(config: Config, args: InsertCategoryArgs) -> Result<Out<String>> {
    // Build the Category object from args
    let category = Category {
        category: args.name.clone(),
        category_group: args.group.unwrap_or_default(),
        r#type: args.r#type.unwrap_or_default(),
        hide_from_reports: args.hide_from_reports.unwrap_or_default(),
        other_fields: args.other_fields,
        original_order: None, // Locally-added rows have no original order
    };

    // Insert into database
    config
        .db()
        .insert_category(&category)
        .await
        .map_err(|e| {
            // Check if this is a unique constraint error
            let err_str = e.to_string();
            if err_str.contains("UNIQUE constraint failed") {
                anyhow::anyhow!("Cannot insert category: '{}' already exists.", args.name)
            } else {
                e
            }
        })
        .pub_result(ErrorType::Database)?;

    let message = format!("Inserted category: {}", args.name);
    Ok(Out::new(message, args.name))
}

/// Inserts a new AutoCat rule into the local SQLite database.
///
/// The primary key is auto-generated (synthetic auto-increment) and returned on success.
///
/// # Arguments
///
/// - `config` - The application configuration containing the database connection.
/// - `args` - The AutoCat rule data to insert. All fields are optional, but a useful rule
///   typically needs at least a category and one or more filter criteria.
///
/// # Returns
///
/// On success, returns an `Out` containing:
/// - A message indicating the AutoCat rule was inserted.
/// - The generated AutoCat ID as a string.
///
/// # Errors
///
/// - Returns an error if a database operation fails.
/// - Returns an error if the specified category does not exist (foreign key constraint).
pub async fn insert_autocat(config: Config, args: InsertAutoCatArgs) -> Result<Out<String>> {
    // Build the AutoCat object from args
    let autocat = AutoCat {
        category: args.category.clone().unwrap_or_default(),
        description: args.description.unwrap_or_default(),
        description_contains: args.description_contains.unwrap_or_default(),
        account_contains: args.account_contains.unwrap_or_default(),
        institution_contains: args.institution_contains.unwrap_or_default(),
        amount_min: args.amount_min,
        amount_max: args.amount_max,
        amount_equals: args.amount_equals,
        description_equals: args.description_equals.unwrap_or_default(),
        description_full: args.description_full.unwrap_or_default(),
        full_description_contains: args.full_description_contains.unwrap_or_default(),
        amount_contains: args.amount_contains.unwrap_or_default(),
        other_fields: args.other_fields.clone(),
        original_order: None, // Locally-added rows have no original order
    };

    // Insert into database and get the generated ID
    let id = config
        .db()
        .insert_autocat(&autocat)
        .await
        .map_err(|e| {
            // Check if this is a foreign key constraint error
            let err_str = e.to_string();
            if err_str.contains("FOREIGN KEY constraint failed") {
                anyhow::anyhow!(
                    "Cannot insert AutoCat rule: category '{}' does not exist. \
                     Create the category first or leave the category field empty.",
                    args.category.as_deref().unwrap_or("")
                )
            } else {
                e
            }
        })
        .pub_result(ErrorType::Database)?;

    let id_str = id.to_string();
    let message = format!("Inserted AutoCat rule with ID: {}", id_str);
    Ok(Out::new(message, id_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Amount;
    use crate::test::TestEnv;

    #[tokio::test]
    async fn test_insert_transaction_success() {
        let env = TestEnv::new().await;

        let args = InsertTransactionArgs {
            date: "2025-01-20".to_string(),
            amount: Amount::new(rust_decimal::Decimal::new(-1250, 2)), // -12.50
            description: Some("Test Purchase".to_string()),
            account: Some("Checking".to_string()),
            account_number: None,
            institution: Some("Test Bank".to_string()),
            month: None,
            week: None,
            full_description: None,
            account_id: None,
            check_number: None,
            date_added: None,
            merchant_name: None,
            category_hint: None,
            category: None, // No category - should work without FK constraint
            note: Some("Test note".to_string()),
            tags: None,
            categorized_date: None,
            statement: None,
            metadata: None,
            other_fields: Default::default(),
        };

        let result = insert_transaction(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Inserted transaction with ID:"));

        // Verify the ID starts with "user-"
        let id = out.structure().unwrap();
        assert!(
            id.starts_with("user-"),
            "Expected ID to start with 'user-', got: {}",
            id
        );

        // Verify the transaction exists in the database
        let txn = env.config().db()._get_transaction(id).await.unwrap();
        assert!(txn.is_some());
        let txn = txn.unwrap();
        assert_eq!(txn.date, "2025-01-20");
        assert_eq!(txn.description, "Test Purchase");
        assert_eq!(txn.note, "Test note");
    }

    #[tokio::test]
    async fn test_insert_transaction_with_valid_category() {
        let env = TestEnv::new().await;
        // Insert test data with categories
        env.insert_test_transaction("temp-txn").await;

        let args = InsertTransactionArgs {
            date: "2025-01-20".to_string(),
            amount: Amount::new(rust_decimal::Decimal::new(-500, 2)), // -5.00
            description: Some("Coffee".to_string()),
            account: None,
            account_number: None,
            institution: None,
            month: None,
            week: None,
            full_description: None,
            account_id: None,
            check_number: None,
            date_added: None,
            merchant_name: None,
            category_hint: None,
            category: Some("Food".to_string()), // "Food" exists from insert_test_transaction
            note: None,
            tags: None,
            categorized_date: None,
            statement: None,
            metadata: None,
            other_fields: Default::default(),
        };

        let result = insert_transaction(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let id = out.structure().unwrap();

        // Verify the transaction has the category
        let txn = env
            .config()
            .db()
            ._get_transaction(id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(txn.category, "Food");
    }

    #[tokio::test]
    async fn test_insert_transaction_with_invalid_category_error() {
        let env = TestEnv::new().await;

        let args = InsertTransactionArgs {
            date: "2025-01-20".to_string(),
            amount: Amount::new(rust_decimal::Decimal::new(-500, 2)),
            description: Some("Test".to_string()),
            account: None,
            account_number: None,
            institution: None,
            month: None,
            week: None,
            full_description: None,
            account_id: None,
            check_number: None,
            date_added: None,
            merchant_name: None,
            category_hint: None,
            category: Some("NonexistentCategory".to_string()),
            note: None,
            tags: None,
            categorized_date: None,
            statement: None,
            metadata: None,
            other_fields: Default::default(),
        };

        let result = insert_transaction(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Cannot insert transaction") || err_msg.contains("FOREIGN KEY"),
            "Expected foreign key error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_insert_transaction_generates_unique_ids() {
        let env = TestEnv::new().await;

        let make_args = || InsertTransactionArgs {
            date: "2025-01-20".to_string(),
            amount: Amount::new(rust_decimal::Decimal::new(-100, 2)),
            description: None,
            account: None,
            account_number: None,
            institution: None,
            month: None,
            week: None,
            full_description: None,
            account_id: None,
            check_number: None,
            date_added: None,
            merchant_name: None,
            category_hint: None,
            category: None,
            note: None,
            tags: None,
            categorized_date: None,
            statement: None,
            metadata: None,
            other_fields: Default::default(),
        };

        let result1 = insert_transaction(env.config(), make_args()).await.unwrap();
        let result2 = insert_transaction(env.config(), make_args()).await.unwrap();

        let id1 = result1.structure().unwrap();
        let id2 = result2.structure().unwrap();

        assert_ne!(id1, id2, "Generated IDs should be unique");
        assert!(id1.starts_with("user-"));
        assert!(id2.starts_with("user-"));
    }

    #[tokio::test]
    async fn test_insert_transaction_minimal_fields() {
        let env = TestEnv::new().await;

        // Only required fields: date and amount
        let args = InsertTransactionArgs {
            date: "2025-01-20".to_string(),
            amount: Amount::new(rust_decimal::Decimal::new(10000, 2)), // 100.00 (positive = income)
            description: None,
            account: None,
            account_number: None,
            institution: None,
            month: None,
            week: None,
            full_description: None,
            account_id: None,
            check_number: None,
            date_added: None,
            merchant_name: None,
            category_hint: None,
            category: None,
            note: None,
            tags: None,
            categorized_date: None,
            statement: None,
            metadata: None,
            other_fields: Default::default(),
        };

        let result = insert_transaction(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let id = out.structure().unwrap();

        // Verify the transaction was created with defaults
        let txn = env
            .config()
            .db()
            ._get_transaction(id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(txn.date, "2025-01-20");
        assert_eq!(txn.description, "");
        assert_eq!(txn.account, "");
    }

    // ==================== insert_category tests ====================

    #[tokio::test]
    async fn test_insert_category_success() {
        let env = TestEnv::new().await;

        let args = InsertCategoryArgs {
            name: "Groceries".to_string(),
            group: Some("Food".to_string()),
            r#type: Some("Expense".to_string()),
            hide_from_reports: None,
            other_fields: Default::default(),
        };

        let result = insert_category(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Inserted category: Groceries"));
        assert_eq!(out.structure().unwrap(), "Groceries");

        // Verify the category exists in the database
        let cat = env.config().db()._get_category("Groceries").await.unwrap();
        assert!(cat.is_some());
        let cat = cat.unwrap();
        assert_eq!(cat.category, "Groceries");
        assert_eq!(cat.category_group, "Food");
        assert_eq!(cat.r#type, "Expense");
    }

    #[tokio::test]
    async fn test_insert_category_minimal_fields() {
        let env = TestEnv::new().await;

        // Only required field: name
        let args = InsertCategoryArgs {
            name: "TestCategory".to_string(),
            group: None,
            r#type: None,
            hide_from_reports: None,
            other_fields: Default::default(),
        };

        let result = insert_category(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert_eq!(out.structure().unwrap(), "TestCategory");

        // Verify the category was created with defaults
        let cat = env
            .config()
            .db()
            ._get_category("TestCategory")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cat.category, "TestCategory");
        assert_eq!(cat.category_group, "");
        assert_eq!(cat.r#type, "");
    }

    #[tokio::test]
    async fn test_insert_category_duplicate_error() {
        let env = TestEnv::new().await;

        let args = InsertCategoryArgs {
            name: "DuplicateCategory".to_string(),
            group: Some("Test".to_string()),
            r#type: None,
            hide_from_reports: None,
            other_fields: Default::default(),
        };

        // First insert should succeed
        let result = insert_category(env.config(), args.clone()).await;
        assert!(result.is_ok());

        // Second insert with same name should fail
        let result = insert_category(env.config(), args).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Cannot insert category")
                || err_msg.contains("already exists")
                || err_msg.contains("UNIQUE constraint failed"),
            "Expected duplicate error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_insert_category_with_hide_from_reports() {
        let env = TestEnv::new().await;

        let args = InsertCategoryArgs {
            name: "HiddenCategory".to_string(),
            group: Some("Internal".to_string()),
            r#type: Some("Transfer".to_string()),
            hide_from_reports: Some("Hide".to_string()),
            other_fields: Default::default(),
        };

        let result = insert_category(env.config(), args).await;

        assert!(result.is_ok());

        // Verify the category has the hide_from_reports field set
        let cat = env
            .config()
            .db()
            ._get_category("HiddenCategory")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cat.hide_from_reports, "Hide");
    }

    // ==================== insert_autocat tests ====================

    #[tokio::test]
    async fn test_insert_autocat_success() {
        let env = TestEnv::new().await;
        // Insert a category first for FK constraint
        env.insert_test_transaction("temp-txn").await;

        let args = InsertAutoCatArgs {
            category: Some("Food".to_string()), // "Food" exists from insert_test_transaction
            description: Some("Starbucks".to_string()),
            description_contains: Some("STARBUCKS".to_string()),
            account_contains: None,
            institution_contains: None,
            amount_min: None,
            amount_max: None,
            amount_equals: None,
            description_equals: None,
            description_full: None,
            full_description_contains: None,
            amount_contains: None,
            other_fields: std::collections::BTreeMap::new(),
        };

        let result = insert_autocat(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        assert!(out.message().contains("Inserted AutoCat rule with ID:"));

        // Verify the ID is a positive integer
        let id_str = out.structure().unwrap();
        let id: u64 = id_str.parse().expect("ID should be a valid number");
        assert!(id > 0, "ID should be positive, got: {}", id);

        // Verify the autocat exists in the database
        let autocat = env.config().db()._get_autocat(id_str).await.unwrap();
        assert!(autocat.is_some());
        let autocat = autocat.unwrap();
        assert_eq!(autocat.row.category, "Food");
        assert_eq!(autocat.row.description, "Starbucks");
        assert_eq!(autocat.row.description_contains, "STARBUCKS");
    }

    #[tokio::test]
    async fn test_insert_autocat_without_category() {
        let env = TestEnv::new().await;

        // Insert without a category - should work (category is optional)
        let args = InsertAutoCatArgs {
            category: None,
            description: Some("Clean description".to_string()),
            description_contains: Some("dirty".to_string()),
            account_contains: None,
            institution_contains: None,
            amount_min: None,
            amount_max: None,
            amount_equals: None,
            description_equals: None,
            description_full: None,
            full_description_contains: None,
            amount_contains: None,
            other_fields: std::collections::BTreeMap::new(),
        };

        let result = insert_autocat(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let id_str = out.structure().unwrap();

        // Verify the autocat was created with empty category
        let autocat = env
            .config()
            .db()
            ._get_autocat(id_str)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(autocat.row.category, "");
        assert_eq!(autocat.row.description, "Clean description");
    }

    #[tokio::test]
    async fn test_insert_autocat_with_invalid_category_error() {
        let env = TestEnv::new().await;

        let args = InsertAutoCatArgs {
            category: Some("NonexistentCategory".to_string()),
            description: None,
            description_contains: Some("test".to_string()),
            account_contains: None,
            institution_contains: None,
            amount_min: None,
            amount_max: None,
            amount_equals: None,
            description_equals: None,
            description_full: None,
            full_description_contains: None,
            amount_contains: None,
            other_fields: std::collections::BTreeMap::new(),
        };

        let result = insert_autocat(env.config(), args).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Cannot insert AutoCat rule") || err_msg.contains("FOREIGN KEY"),
            "Expected foreign key error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_insert_autocat_generates_unique_ids() {
        let env = TestEnv::new().await;

        let make_args = || InsertAutoCatArgs {
            category: None,
            description: None,
            description_contains: Some("test".to_string()),
            account_contains: None,
            institution_contains: None,
            amount_min: None,
            amount_max: None,
            amount_equals: None,
            description_equals: None,
            description_full: None,
            full_description_contains: None,
            amount_contains: None,
            other_fields: std::collections::BTreeMap::new(),
        };

        let result1 = insert_autocat(env.config(), make_args()).await.unwrap();
        let result2 = insert_autocat(env.config(), make_args()).await.unwrap();

        let id1 = result1.structure().unwrap();
        let id2 = result2.structure().unwrap();

        assert_ne!(id1, id2, "Generated IDs should be unique");
    }

    #[tokio::test]
    async fn test_insert_autocat_minimal_fields() {
        let env = TestEnv::new().await;

        // All fields are optional - insert with defaults
        let args = InsertAutoCatArgs {
            category: None,
            description: None,
            description_contains: None,
            account_contains: None,
            institution_contains: None,
            amount_min: None,
            amount_max: None,
            amount_equals: None,
            description_equals: None,
            description_full: None,
            full_description_contains: None,
            amount_contains: None,
            other_fields: std::collections::BTreeMap::new(),
        };

        let result = insert_autocat(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let id_str = out.structure().unwrap();

        // Verify the autocat was created with empty defaults
        let autocat = env
            .config()
            .db()
            ._get_autocat(id_str)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(autocat.row.category, "");
        assert_eq!(autocat.row.description, "");
        assert_eq!(autocat.row.description_contains, "");
    }

    #[tokio::test]
    async fn test_insert_autocat_with_amount_filters() {
        let env = TestEnv::new().await;

        let args = InsertAutoCatArgs {
            category: None,
            description: None,
            description_contains: None,
            account_contains: None,
            institution_contains: None,
            amount_min: Some(Amount::new(rust_decimal::Decimal::new(1000, 2))), // 10.00
            amount_max: Some(Amount::new(rust_decimal::Decimal::new(5000, 2))), // 50.00
            amount_equals: None,
            description_equals: None,
            description_full: None,
            full_description_contains: None,
            amount_contains: None,
            other_fields: std::collections::BTreeMap::new(),
        };

        let result = insert_autocat(env.config(), args).await;

        assert!(result.is_ok());
        let out = result.unwrap();
        let id_str = out.structure().unwrap();

        // Verify the amount filters were stored
        let autocat = env
            .config()
            .db()
            ._get_autocat(id_str)
            .await
            .unwrap()
            .unwrap();
        assert!(autocat.row.amount_min.is_some());
        assert!(autocat.row.amount_max.is_some());
    }
}
