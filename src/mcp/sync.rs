//! Implementation of the sync_up and sync_down commands for MCP

use crate::commands;
use crate::commands::FormulasMode;
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
    /// Initialize the tiller MCP service for this session and return usage instructions. You \
    /// **MUST** call this **ONCE** before using other tools so that you have the full usage \
    /// instructions. You **MAY** call it more than once if you have forgotten the usage \
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
    ///    for conflict detection during future `sync up` operations.
    ///
    /// # Database Updates
    ///
    /// - **Transactions**: Upsert semantics. New rows are inserted, existing rows are updated,
    ///   and rows no longer in the sheet are deleted. Each row's `original_order` is set to its
    ///   0-indexed position from the sheet.
    /// - **Categories and AutoCat**: Full replacement. All existing rows are deleted, then all
    ///   sheet rows are inserted.
    /// - **Formulas**: Cell formulas are captured and stored in the `formulas` table for optional
    ///   preservation during `sync up`.
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
        match commands::sync_down(config, self.mode).await {
            Ok(message) => Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                message,
            )])),
            Err(e) => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                format!("sync_down failed: {e}"),
            )])),
        }
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
        match commands::sync_up(config, self.mode, params.force, params.formulas).await {
            Ok(message) => Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                message,
            )])),
            Err(e) => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                format!("sync_up failed: {e}"),
            )])),
        }
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
