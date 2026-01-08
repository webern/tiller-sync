//! Implementation of the sync_up and sync_down commands for MCP

use crate::args::{
    DeleteAutoCatsArgs, DeleteCategoriesArgs, DeleteTransactionsArgs, InsertAutoCatArgs,
    InsertCategoryArgs, InsertTransactionArgs, UpdateAutoCatsArgs, UpdateCategoriesArgs,
    UpdateTransactionsArgs,
};
use crate::commands::{self, FormulasMode};
use crate::mcp::mcp_utils::tool_result;
use crate::mcp::TillerServer;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use tracing::info;

/// Parameters for the sync_up tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(title = "SyncUpParams")]
pub struct SyncUpParams {
    /// Force sync even if conflicts are detected or sync-down backup is missing. Use with caution
    /// as this may overwrite remote changes.
    #[serde(default)]
    pub force: bool,

    /// How to handle formulas: 'unknown' (error if formulas exist), 'preserve' (write formulas
    /// back), or 'ignore' (skip formulas, write values only). Default is 'unknown'.
    #[serde(default)]
    pub formulas: FormulasMode,
}

#[tool_router(vis = "pub(super)")]
impl TillerServer {
    #[tool]
    /// Initialize the tiller MCP service for this session and return usage instructions. You
    /// **MUST** call this **ONCE** before using other tools so that you have the full usage
    /// instructions. You **MAY** call it more than once if you have forgotten the usage
    /// instructions.
    async fn initialize_service(&self) -> Result<CallToolResult, McpError> {
        let mut initialized = self.initialized.lock().await;
        *initialized = true;
        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            include_str!("docs/INSTRUCTIONS.md"),
        )]))
    }

    /// Download Transactions, Categories, and AutoCat data from the configured Tiller Google
    /// Sheet to the local SQLite database. Creates a backup first.
    ///
    /// # Backup Procedures (Automatic)
    ///
    /// Before any writes:
    ///
    /// 1. **SQLite backup** (`tiller.sqlite.YYYY-MM-DD-NNN`): Timestamped copy of the existing
    ///    database (if it exists).
    /// 2. **JSON snapshot** (`sync-down.YYYY-MM-DD-NNN.json`): Captures the downloaded sheet data
    ///    for conflict detection during future `sync_up` operations.
    ///
    /// # Database Updates
    ///
    /// - **Transactions**: Upsert semantics. New rows are inserted, existing rows are updated,
    ///   and rows no longer in the sheet are deleted. Each row's `original_order` is set to its
    ///   0-indexed position from the sheet.
    /// - **Categories and AutoCat**: Full replacement. All existing rows are deleted, then all
    ///   sheet rows are inserted.
    /// - **Formulas**: Cell formulas are captured and stored in the `formulas` table for optional
    ///   preservation during `sync_up`.
    ///
    /// # Caution
    ///
    /// This operation overwrites local changes with sheet data. If you have local modifications
    /// that haven't been synced up, they will be lost. The SQLite backup allows manual recovery
    /// if needed.
    #[tool]
    async fn sync_down(&self) -> Result<CallToolResult, McpError> {
        require_init!(self);
        info!("MCP: sync_down called");
        let config = (*self.config).clone();
        let out = commands::sync_down(config, self.mode).await;
        tool_result(out)
    }

    /// Upload Transactions, Categories, and AutoCat data from the local SQLite database to the
    /// Google Sheet. Creates backups before writing. Use 'force' to override conflict detection,
    /// 'formulas' to control formula handling.
    ///
    /// # Strategy
    ///
    /// This command treats the local SQLite database as the authoritative source of truth and
    /// completely replaces the Google Sheet contents using a clear-and-write approach.
    ///
    /// # Backup Procedures (Automatic)
    ///
    /// Before any destructive writes, the following backups are created:
    ///
    /// 1. **Pre-upload JSON snapshot** (`sync-up-pre.YYYY-MM-DD-NNN.json`): Captures the current
    ///    state of the Google Sheet before modification.
    /// 2. **SQLite backup** (`tiller.sqlite.YYYY-MM-DD-NNN`): Creates a timestamped copy of the
    ///    local database.
    /// 3. **Google Sheet copy**: Uses the Drive API to create a full copy of the spreadsheet
    ///    named `tiller-backup-YYYY-MM-DD-HHMMSS`.
    ///
    /// # Conflict Detection
    ///
    /// Before uploading, the tool compares the current Google Sheet state against the last
    /// `sync-down` backup. If differences are detected (indicating the sheet was modified since
    /// last download):
    ///
    /// - **Without `force`**: Returns an error recommending `sync down` first to merge changes.
    /// - **With `force=true`**: Proceeds with upload, overwriting any remote changes.
    ///
    /// If no `sync-down` backup exists:
    ///
    /// - **Without `force`**: Returns an error recommending `sync down` first.
    /// - **With `force=true`**: Skips conflict detection entirely.
    ///
    /// # Formula Handling
    ///
    /// Tiller sheets may contain formulas (e.g., `=SUM(...)` in balance columns). The `formulas`
    /// parameter controls how these are handled:
    ///
    /// - **`unknown`** (default): If formulas exist in the database, returns an error prompting
    ///   the user to explicitly choose `preserve` or `ignore`.
    /// - **`preserve`**: Writes formulas back to their original cell positions. This uses the
    ///   `original_order` column to maintain row alignment.
    /// - **`ignore`**: Skips all formulas; only values are written to the sheet.
    ///
    /// ## Formula Preservation Caveats
    ///
    /// When `formulas=preserve` is used and rows have been deleted locally (detected as gaps in
    /// `original_order`), formulas may reference incorrect cells because row positions have
    /// shifted:
    ///
    /// - **Without `force`**: Returns an error explaining that formula positions may be corrupted.
    /// - **With `force=true`**: Proceeds anyway, writing formulas to their original positions.
    ///
    /// # Preconditions
    ///
    /// - The local database must contain transactions. Run `sync down` first if empty.
    /// - Authentication must be valid.
    ///
    /// # Verification
    ///
    /// After writing, the tool re-fetches row counts from each sheet tab and verifies they match
    /// what was written.
    #[tool]
    async fn sync_up(
        &self,
        Parameters(params): Parameters<SyncUpParams>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        info!(
            "MCP: sync_up called with force={}, formulas={}",
            params.force, params.formulas
        );

        let config = (*self.config).clone();
        let out = commands::sync_up(config, self.mode, params.force, params.formulas).await;
        tool_result(out)
    }

    /// Update one or more transactions in the local database by their IDs.
    ///
    /// This tool modifies transaction fields in the local SQLite database. When more than one ID
    /// is provided, all specified transactions receive the same updates. Changes are NOT
    /// automatically synced to the Google Sheet - call `sync_up` to upload local changes.
    ///
    /// # Parameters
    ///
    /// - `ids`: One or more transaction IDs to update. All specified transactions will receive the
    ///   same field updates.
    /// - `updates`: The fields to update. Only fields with values will be modified; unspecified
    ///   fields remain unchanged. See `TransactionUpdates` for available fields.
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating how many transactions were updated and a JSON array
    /// of the updated transaction objects.
    ///
    /// # Example
    ///
    /// Update one transaction:
    ///
    /// ```json
    /// {
    ///   "ids": ["abc123"],
    ///   "category": "Groceries",
    ///   "note": "Weekly shopping"
    /// }
    /// ```
    ///
    /// Update more than one transaction with the same values:
    ///
    /// ```json
    /// {
    ///   "ids": ["abc123", "def456"],
    ///   "category": "Entertainment"
    /// }
    /// ```
    #[tool]
    async fn update_transactions(
        &self,
        Parameters(args): Parameters<UpdateTransactionsArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::update_transactions(config, args).await;
        tool_result(out)
    }

    /// Update one or more categories in the local database by their names.
    ///
    /// This tool modifies category fields in the local SQLite database. The category name is the
    /// primary key. To rename a category, provide the current name and include the new name in the
    /// updates. Changes are NOT automatically synced to the Google Sheet - call `sync_up` to
    /// upload local changes. When updating multiple categories, the operation is atomic: either
    /// all updates succeed or none do.
    ///
    /// # Renaming Categories
    ///
    /// Due to `ON UPDATE CASCADE` foreign key constraints, renaming a category automatically
    /// updates all references in transactions and autocat rules. This is a safe operation that
    /// maintains data integrity.
    ///
    /// # Parameters
    ///
    /// - `name`: The name of the category to update (this is the primary key).
    /// - `updates`: The fields to update. Only fields with values will be modified; unspecified
    ///   fields remain unchanged. See `CategoryUpdates` for available fields:
    ///   - `category`: New name for the category (renames it)
    ///   - `group`: The group this category belongs to (e.g., "Food", "Transportation")
    ///   - `type`: Category type ("Expense", "Income", or "Transfer")
    ///   - `hide_from_reports`: Set to "Hide" to exclude from reports
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating the category was updated and a JSON object of
    /// the updated category.
    ///
    /// # Example
    ///
    /// Update a category's group:
    ///
    /// ```json
    /// {
    ///   "name": "Groceries",
    ///   "group": "Food & Dining"
    /// }
    /// ```
    ///
    /// Rename a category:
    ///
    /// ```json
    /// {
    ///   "name": "Food",
    ///   "category": "Groceries"
    /// }
    /// ```
    #[tool]
    async fn update_categories(
        &self,
        Parameters(args): Parameters<UpdateCategoriesArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::update_categories(config, args).await;
        tool_result(out)
    }

    /// Update one or more AutoCat rules in the local database by their IDs.
    ///
    /// This tool modifies AutoCat rule fields in the local SQLite database. AutoCat rules have a
    /// synthetic auto-increment primary key that is assigned when first synced down or inserted
    /// locally. Changes are NOT automatically synced to the Google Sheet - call `sync_up` to
    /// upload local changes. When updating multiple rules, the operation is atomic: either all
    /// updates succeed or none do.
    ///
    /// # AutoCat Overview
    ///
    /// AutoCat rules automatically categorize transactions based on matching criteria. Rules are
    /// processed sequentially from top to bottom, so organize specific rules above broader ones.
    ///
    /// # Parameters
    ///
    /// - `id`: The ID of the AutoCat rule to update (synthetic auto-increment primary key).
    /// - `updates`: The fields to update. Only fields with values will be modified; unspecified
    ///   fields remain unchanged. Available fields:
    ///
    ///   **Override columns** (applied when rule matches):
    ///   - `category`: The category to assign to matching transactions
    ///   - `description`: Override to standardize transaction descriptions
    ///
    ///   **Filter criteria** (all non-blank criteria are AND-ed):
    ///   - `description_contains`: Text to search for in Description (case-insensitive, supports
    ///     multiple comma-separated keywords in quotes that are OR-ed)
    ///   - `account_contains`: Text to search for in Account column
    ///   - `institution_contains`: Text to search for in Institution column
    ///   - `amount_min`: Minimum amount (absolute value)
    ///   - `amount_max`: Maximum amount (absolute value)
    ///   - `amount_equals`: Exact amount to match
    ///   - `description_equals`: Exact match for Description
    ///   - `full_description_contains`: Text to search for in Full Description
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating the rule was updated and a JSON object of
    /// the updated AutoCat rule (including its ID).
    ///
    /// # Example
    ///
    /// Update an AutoCat rule's filter criteria:
    ///
    /// ```json
    /// {
    ///   "id": "1",
    ///   "description_contains": "starbucks,coffee shop",
    ///   "category": "Food & Dining"
    /// }
    /// ```
    #[tool]
    async fn update_autocats(
        &self,
        Parameters(args): Parameters<UpdateAutoCatsArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::update_autocats(config, args).await;
        tool_result(out)
    }

    /// Delete one or more transactions from the local database by their IDs.
    ///
    /// This tool permanently removes transactions from the local SQLite database. Changes are NOT
    /// automatically synced to the Google Sheet - call `sync_up` to upload local changes.
    ///
    /// # Warning
    ///
    /// This operation cannot be undone locally. However, if you haven't run `sync_up` yet, you can
    /// restore the transactions by running `sync_down` to re-download from the sheet.
    ///
    /// # Parameters
    ///
    /// - `ids`: One or more transaction IDs to delete. All specified transactions will be removed.
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating how many transactions were deleted and a JSON array
    /// of the deleted transaction IDs.
    ///
    /// # Errors
    ///
    /// - Returns an error if a transaction ID is not found.
    /// - The operation is atomic: if any error occurs, all changes are rolled back and no
    ///   transactions are deleted.
    ///
    /// # Example
    ///
    /// Delete a single transaction:
    ///
    /// ```json
    /// {
    ///   "ids": ["abc123"]
    /// }
    /// ```
    ///
    /// Delete multiple transactions:
    ///
    /// ```json
    /// {
    ///   "ids": ["abc123", "def456", "ghi789"]
    /// }
    /// ```
    #[tool]
    async fn delete_transactions(
        &self,
        Parameters(args): Parameters<DeleteTransactionsArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::delete_transactions(config, args).await;
        tool_result(out)
    }

    /// Delete one or more categories from the local database by their names.
    ///
    /// This tool permanently removes categories from the local SQLite database. Changes are NOT
    /// automatically synced to the Google Sheet - call `sync_up` to upload local changes.
    ///
    /// # Foreign Key Constraints
    ///
    /// Due to `ON DELETE RESTRICT` foreign key constraints, a category cannot be deleted if any
    /// transactions or AutoCat rules reference it. Those references must be updated or deleted
    /// first using `update_transactions`, `update_autocats`, or `delete_transactions`.
    ///
    /// # Warning
    ///
    /// This operation cannot be undone locally. However, if you haven't run `sync_up` yet, you can
    /// restore the categories by running `sync_down` to re-download from the sheet.
    ///
    /// # Parameters
    ///
    /// - `names`: One or more category names to delete.
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating how many categories were deleted and a JSON array
    /// of the deleted category names.
    ///
    /// # Errors
    ///
    /// - Returns an error if a category is not found.
    /// - Returns an error if a foreign key constraint prevents deletion.
    /// - The operation is atomic: if any error occurs, all changes are rolled back and no
    ///   categories are deleted.
    ///
    /// # Example
    ///
    /// Delete a single category:
    ///
    /// ```json
    /// {
    ///   "names": ["Old Category"]
    /// }
    /// ```
    ///
    /// Delete multiple categories:
    ///
    /// ```json
    /// {
    ///   "names": ["Category1", "Category2", "Category3"]
    /// }
    /// ```
    #[tool]
    async fn delete_categories(
        &self,
        Parameters(args): Parameters<DeleteCategoriesArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::delete_categories(config, args).await;
        tool_result(out)
    }

    /// Delete one or more AutoCat rules from the local database by their IDs.
    ///
    /// This tool permanently removes AutoCat rules from the local SQLite database. Changes are NOT
    /// automatically synced to the Google Sheet - call `sync_up` to upload local changes.
    ///
    /// AutoCat rules have synthetic auto-increment IDs assigned when first synced down or inserted
    /// locally.
    ///
    /// # Warning
    ///
    /// This operation cannot be undone locally. However, if you haven't run `sync_up` yet, you can
    /// restore the rules by running `sync_down` to re-download from the sheet.
    ///
    /// # Parameters
    ///
    /// - `ids`: One or more AutoCat rule IDs to delete.
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating how many AutoCat rules were deleted and a JSON
    /// array of the deleted rule IDs.
    ///
    /// # Errors
    ///
    /// - Returns an error if an AutoCat rule ID is not found.
    /// - The operation is atomic: if any error occurs, all changes are rolled back and no
    ///   rules are deleted.
    ///
    /// # Example
    ///
    /// Delete a single AutoCat rule:
    ///
    /// ```json
    /// {
    ///   "ids": ["1"]
    /// }
    /// ```
    ///
    /// Delete multiple AutoCat rules:
    ///
    /// ```json
    /// {
    ///   "ids": ["1", "2", "3"]
    /// }
    /// ```
    #[tool]
    async fn delete_autocats(
        &self,
        Parameters(args): Parameters<DeleteAutoCatsArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::delete_autocats(config, args).await;
        tool_result(out)
    }

    /// Insert a new transaction into the local database.
    ///
    /// This tool creates a new transaction in the local SQLite database. A unique transaction ID
    /// is automatically generated with a `user-` prefix to distinguish it from Tiller-created
    /// transactions. The generated ID is returned on success. Changes are NOT automatically synced
    /// to the Google Sheet - call `sync_up` to upload local changes.
    ///
    /// # Parameters
    ///
    /// - `date`: The posted date (when the transaction cleared) or transaction date. **Required.**
    /// - `amount`: Transaction value where income and credits are positive; expenses and debits
    ///   are negative. **Required.**
    /// - All other fields are optional. See the `InsertTransactionArgs` schema for the full list.
    ///
    /// # Foreign Key Constraints
    ///
    /// If a `category` is specified, it must reference an existing category in the database.
    /// The insert will fail if the category does not exist. Either create the category first
    /// or leave the category field empty.
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating the transaction was inserted and the generated
    /// transaction ID.
    ///
    /// # Example
    ///
    /// Insert a transaction with minimal fields:
    ///
    /// ```json
    /// {
    ///   "date": "2025-01-20",
    ///   "amount": "-25.50"
    /// }
    /// ```
    ///
    /// Insert a transaction with more details:
    ///
    /// ```json
    /// {
    ///   "date": "2025-01-20",
    ///   "amount": "-25.50",
    ///   "description": "Coffee Shop",
    ///   "account": "Checking",
    ///   "category": "Food",
    ///   "note": "Morning coffee"
    /// }
    /// ```
    #[tool]
    async fn insert_transaction(
        &self,
        Parameters(args): Parameters<InsertTransactionArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::insert_transaction(config, args).await;
        tool_result(out)
    }

    /// Insert a new category into the local database.
    ///
    /// This tool creates a new category in the local SQLite database. The category name is the
    /// primary key and must be unique. The name is returned on success. Changes are NOT
    /// automatically synced to the Google Sheet - call `sync_up` to upload local changes.
    ///
    /// # Parameters
    ///
    /// - `name`: The name of the category. This is the primary key and must be unique. **Required.**
    /// - `group`: The group this category belongs to (e.g., "Food", "Transportation").
    /// - `type`: Category type classification ("Expense", "Income", or "Transfer").
    /// - `hide_from_reports`: Set to "Hide" to exclude this category from reports.
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating the category was inserted and the category name.
    ///
    /// # Errors
    ///
    /// - Returns an error if a category with the same name already exists.
    ///
    /// # Example
    ///
    /// Insert a category with minimal fields:
    ///
    /// ```json
    /// {
    ///   "name": "Groceries"
    /// }
    /// ```
    ///
    /// Insert a category with all fields:
    ///
    /// ```json
    /// {
    ///   "name": "Groceries",
    ///   "group": "Food",
    ///   "type": "Expense",
    ///   "hide_from_reports": ""
    /// }
    /// ```
    #[tool]
    async fn insert_category(
        &self,
        Parameters(args): Parameters<InsertCategoryArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::insert_category(config, args).await;
        tool_result(out)
    }

    /// Insert a new AutoCat rule into the local database.
    ///
    /// This tool creates a new AutoCat rule in the local SQLite database. AutoCat rules define
    /// automatic categorization criteria for transactions. The primary key is auto-generated
    /// (synthetic auto-increment) and returned on success. Changes are NOT automatically synced
    /// to the Google Sheet - call `sync_up` to upload local changes.
    ///
    /// # AutoCat Overview
    ///
    /// AutoCat rules automatically categorize transactions based on matching criteria. Rules are
    /// processed sequentially from top to bottom, so organize specific rules above broader ones.
    ///
    /// # Parameters
    ///
    /// All fields are optional - an empty rule can be created and updated later. However, a useful
    /// rule typically needs at least a category and one or more filter criteria.
    ///
    /// **Override columns** (applied when rule matches):
    /// - `category`: The category to assign to matching transactions. Must reference an existing
    ///   category.
    /// - `description`: Override to standardize transaction descriptions (e.g., replace
    ///   "Seattle Starbucks store 1234" with simply "Starbucks").
    ///
    /// **Filter criteria** (all non-blank criteria are AND-ed):
    /// - `description_contains`: Text to search for in Description (case-insensitive, supports
    ///   multiple comma-separated keywords in quotes that are OR-ed).
    /// - `account_contains`: Text to search for in Account column.
    /// - `institution_contains`: Text to search for in Institution column.
    /// - `amount_min`: Minimum amount (absolute value).
    /// - `amount_max`: Maximum amount (absolute value).
    /// - `amount_equals`: Exact amount to match.
    /// - `description_equals`: Exact match for Description.
    /// - `description_full`: Override column for the full/raw description field.
    /// - `full_description_contains`: Text to search for in Full Description.
    /// - `amount_contains`: Text pattern to search for in Amount column.
    ///
    /// # Foreign Key Constraints
    ///
    /// If a `category` is specified, it must reference an existing category in the database.
    /// The insert will fail if the category does not exist. Either create the category first
    /// or leave the category field empty.
    ///
    /// # Returns
    ///
    /// On success, returns a message indicating the AutoCat rule was inserted and the generated
    /// rule ID.
    ///
    /// # Example
    ///
    /// Insert a simple AutoCat rule:
    ///
    /// ```json
    /// {
    ///   "category": "Food & Dining",
    ///   "description_contains": "starbucks,coffee"
    /// }
    /// ```
    ///
    /// Insert an AutoCat rule with amount filter:
    ///
    /// ```json
    /// {
    ///   "category": "Subscriptions",
    ///   "description_contains": "netflix",
    ///   "amount_min": "10.00",
    ///   "amount_max": "20.00"
    /// }
    /// ```
    #[tool]
    async fn insert_autocat(
        &self,
        Parameters(args): Parameters<InsertAutoCatArgs>,
    ) -> Result<CallToolResult, McpError> {
        require_init!(self);

        let config = (*self.config).clone();
        let out = commands::insert_autocat(config, args).await;
        tool_result(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that tool descriptions can be retrieved from the generated metadata functions.
    /// This test verifies that when no explicit `description` attribute is provided,
    /// doc comments above the `#[tool]` macro are used as the description.
    #[test]
    fn test_tool_descriptions_from_doc_comments() {
        // sync_down uses doc comments for its description (no explicit description attribute)
        let sync_down_tool = TillerServer::sync_down_tool_attr();
        let description = sync_down_tool
            .description
            .expect("sync_down should have a description");

        // The description should come from doc comments
        assert!(
            description.contains("Download"),
            "Expected description from doc comments, got: {description}"
        );

        // sync_up uses an explicit description attribute
        let sync_up_tool = TillerServer::sync_up_tool_attr();
        let description = sync_up_tool
            .description
            .expect("sync_up should have a description");
        assert!(
            description.contains("Upload"),
            "Expected explicit description, got: {description}"
        );
    }

    /// A test that verifies doc comments are being presented in the JSON schema.
    #[test]
    fn sync_up_params_schema_description() {
        let schema_object = schemars::schema_for!(SyncUpParams);
        let schema = serde_json::to_string_pretty(&schema_object).unwrap();
        let expected_snippet = "error if formulas exist";
        let contains_snippet = schema.contains(expected_snippet);
        assert!(
            contains_snippet,
            "Expected JSON schema to contain '{expected_snippet}' \
        but it did not. Schema:\n\n{schema}\n\n"
        );
    }
}
