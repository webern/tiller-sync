//! These structs provide the CLI interface for the tiller CLI.

use crate::commands::FormulasMode;
use crate::error::{ErrorType, IntoResult};
use crate::model::{Amount, AutoCatUpdates, CategoryUpdates, TransactionUpdates};
use crate::utils;
use crate::Result;
use anyhow::anyhow;
use clap::{Parser, Subcommand};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::error;
use tracing_subscriber::filter::LevelFilter;

/// tiller: A command-line tool for manipulating financial data.
///
/// The purpose of this program is to download your financial transactions from a Tiller Google
/// sheet (see https://tiller.com) into a local datastore. There you can manipulate tham as you
/// wish and then sync your changes back to your Tiller sheet.
///
/// You will need set up a Google Docs API Key and OAuth for this. See the README at
/// https://github.com/webern/tiller-sync for documentation on how to set this up.
///
/// There is also a mode in which an AI agent, like Claude or Claude Code, can use this program
/// through the mcp subcommand.
#[derive(Debug, Parser, Clone)]
pub struct Args {
    #[clap(flatten)]
    common: Common,

    #[command(subcommand)]
    command: Command,
}

impl Args {
    pub fn new(common: Common, command: Command) -> Self {
        Self { common, command }
    }

    pub fn common(&self) -> &Common {
        &self.common
    }

    pub fn command(&self) -> &Command {
        &self.command
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Create the data directory and initialize the configuration files.
    ///
    /// This is the first command you should run when setting up the tiller CLI. You need to get a
    /// few things ready beforehand.
    ///
    /// - Decide what directory you want to store data in and pass this as --tiller-home. By
    ///   default, It will be $HOME/tiller. If you want it somewhere else then you should specify
    ///   it.
    ///
    /// - Get the URL of your Tiller Google Sheet and pass it as --sheet-url.
    ///
    /// - Set up your Google Sheets API Access credentials and download them to a file. You will
    ///   pass this as --api-key. Unfortunately, this is a process that requires a lot of steps.
    ///   Detailed instructions have been provided in the GitHub documentation, please see
    ///   https://github.com/webern/tiller-sync for help with this.
    ///
    Init(InitArgs),
    /// Authenticate with Google Sheets via OAuth.
    Auth(AuthArgs),
    /// Upload or Download Transactions, Categories and AutoCat tabs to/from your Tiller Sheet.
    Sync(SyncArgs),
    /// Run as an MCP (Model Context Protocol) server for AI agent integration.
    ///
    /// This launches a long-running process that communicates via JSON-RPC over stdin/stdout.
    /// MCP clients (like Claude Code) launch this as a subprocess.
    Mcp(McpArgs),
    /// Update a transaction, category, or autocat rule in the local database.
    Update(Box<UpdateArgs>),
    /// Delete a transaction, category, or autocat rule from the local database.
    Delete(DeleteArgs),
    /// Insert a new transaction, category, or autocat rule into the local database.
    Insert(Box<InsertArgs>),
}

/// Arguments common to all subcommands.
#[derive(Debug, Parser, Clone)]
pub struct Common {
    /// The logging verbosity. One of, from least to most verbose:
    /// off, error, warn, info, debug, trace
    ///
    /// This can be overridden by the RUST_LOG environment variable.
    #[arg(long, default_value_t = LevelFilter::INFO)]
    log_level: LevelFilter,

    /// The directory where tiller data and configuration is held. Defaults to ~/tiller
    #[arg(long, env = "TILLER_HOME", default_value_t = default_tiller_home())]
    tiller_home: DisplayPath,
}

impl Common {
    pub fn new(log_level: LevelFilter, tiller_home: PathBuf) -> Self {
        Self {
            log_level,
            tiller_home: tiller_home.into(),
        }
    }

    pub fn log_level(&self) -> LevelFilter {
        self.log_level
    }

    pub fn tiller_home(&self) -> &DisplayPath {
        &self.tiller_home
    }
}

/// (Not shown): Args for the `tiller init` command.
#[derive(Debug, Parser, Clone)]
pub struct InitArgs {
    /// The URL to your Tiller Google sheet. It looks like this:
    /// https://docs.google.com/spreadsheets/d/1a7Km9FxQwRbPt82JvN4LzYpH5OcGnWsT6iDuE3VhMjX
    #[arg(long)]
    sheet_url: String,

    /// The path to your downloaded OAuth 2.0 client credentials. This file will be copied to the
    /// default secrets location in the main data directory.
    #[arg(long)]
    client_secret: PathBuf,
}

impl InitArgs {
    pub fn new(sheet_url: impl Into<String>, secret: impl Into<PathBuf>) -> Self {
        Self {
            sheet_url: sheet_url.into(),
            client_secret: secret.into(),
        }
    }

    pub fn sheet_url(&self) -> &str {
        &self.sheet_url
    }

    pub fn client_secret(&self) -> &Path {
        &self.client_secret
    }
}

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpDown {
    Up,
    #[default]
    Down,
}

serde_plain::derive_display_from_serialize!(UpDown);
serde_plain::derive_fromstr_from_deserialize!(UpDown);

/// (Not shown): Args for the `tiller auth` command.
#[derive(Debug, Parser, Clone)]
pub struct AuthArgs {
    /// Verify and refresh authentication.
    #[arg(long)]
    verify: bool,
}

impl AuthArgs {
    pub fn new(verify: bool) -> Self {
        Self { verify }
    }

    pub fn verify(&self) -> bool {
        self.verify
    }
}

/// (Not shown): Args for the `tiller sync` command.
#[derive(Debug, Parser, Clone)]
pub struct SyncArgs {
    /// The direction to sync: "up" or "down"
    direction: UpDown,

    /// The path to the OAuth 2.0 client credentials file, defaults to $TILLER_HOME/.secrets/client_secret.json
    client_secret: Option<PathBuf>,

    /// The path to the Google OAuth token file, defaults to $TILLER_HOME/.secrets/token.json
    oauth_token: Option<PathBuf>,

    /// Force sync up even if conflicts are detected or sync-down backup is missing
    #[arg(long)]
    force: bool,

    /// How to handle formulas during sync up: unknown, preserve, or ignore.
    /// - unknown: Error if formulas exist (default)
    /// - preserve: Write formulas back to original positions
    /// - ignore: Skip all formulas, only write values
    #[arg(long, value_enum, default_value_t = FormulasMode::Unknown)]
    formulas: FormulasMode,
}

impl SyncArgs {
    pub fn new(direction: UpDown, secret: Option<PathBuf>, oath_token: Option<PathBuf>) -> Self {
        Self {
            direction,
            client_secret: secret,
            oauth_token: oath_token,
            force: false,
            formulas: FormulasMode::Unknown,
        }
    }

    pub fn direction(&self) -> UpDown {
        self.direction
    }

    pub fn client_secret(&self) -> Option<&PathBuf> {
        self.client_secret.as_ref()
    }

    pub fn oath_token(&self) -> Option<&PathBuf> {
        self.oauth_token.as_ref()
    }

    pub fn force(&self) -> bool {
        self.force
    }

    pub fn formulas(&self) -> FormulasMode {
        self.formulas
    }
}

/// Args for the `tiller mcp` command.
#[derive(Debug, Parser, Clone, Default)]
pub struct McpArgs {
    // No additional arguments for now.
    // The --tiller-home flag is inherited from Common.
}

/// Args for the `tiller update` command.
#[derive(Debug, Parser, Clone)]
pub struct UpdateArgs {
    #[command(subcommand)]
    entity: UpdateSubcommand,
}

impl UpdateArgs {
    pub fn entity(&self) -> &UpdateSubcommand {
        &self.entity
    }
}

/// Subcommands for `tiller update`.
#[derive(Subcommand, Debug, Clone)]
pub enum UpdateSubcommand {
    /// Updates one or more transactions in the local SQLite database by their IDs. At least one
    /// transaction ID must be provided. When more than one ID is provided, all specified
    /// transactions are updated with the same field values.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    Transactions(Box<UpdateTransactionsArgs>),

    /// Updates one or more categories in the local SQLite database by their names. At least one
    /// category name must be provided. When more than one name is provided, all specified
    /// categories are updated with the same field values.
    ///
    /// Due to `ON UPDATE CASCADE` foreign key constraints, renaming a category automatically
    /// updates all references in transactions and autocat rules.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    Categories(UpdateCategoriesArgs),

    /// Updates one or more AutoCat rules in the local SQLite database by their IDs. At least one
    /// ID must be provided. When more than one ID is provided, all specified rules are updated
    /// with the same field values.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    Autocats(UpdateAutoCatsArgs),
}

/// Args for the `tiller update transactions` command.
///
/// Updates one or more transactions in the local SQLite database by their IDs. At least one
/// transaction ID must be provided. When more than one ID is provided, all specified
/// transactions are updated with the same field values.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
#[derive(Debug, Parser, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateTransactionsArgs {
    /// One or more transaction IDs to update. All specified transactions will receive the same
    /// updates.
    #[arg(long, num_args = 1..)]
    ids: Vec<String>,

    /// The fields to update. Only fields with values will be modified; unspecified fields remain
    /// unchanged.
    #[clap(flatten)]
    updates: TransactionUpdates,
}

impl UpdateTransactionsArgs {
    pub fn new<S, I>(ids: I, updates: TransactionUpdates) -> Result<Self>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let ids: Vec<String> = ids.into_iter().map(|s| s.into()).collect();
        if ids.is_empty() {
            return Err(anyhow!("At least one ID is required")).pub_result(ErrorType::Request);
        }
        Ok(Self { ids, updates })
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn updates(&self) -> &TransactionUpdates {
        &self.updates
    }
}

/// Args for the `tiller update categories` command.
///
/// Updates one or more categories in the local SQLite database by their names. At least one
/// category name must be provided. When more than one name is provided, all specified
/// categories are updated with the same field values.
///
/// The category name is the primary key. To rename a category, provide a single name and set
/// the `--category` update field to the new name.
///
/// Due to `ON UPDATE CASCADE` foreign key constraints, renaming a category automatically
/// updates all references in transactions and autocat rules.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
#[derive(Debug, Parser, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateCategoriesArgs {
    /// One or more category names to update. All specified categories will receive the same
    /// updates.
    #[arg(long, num_args = 1..)]
    names: Vec<String>,

    /// The fields to update. Only fields with values will be modified; unspecified fields remain
    /// unchanged.
    #[clap(flatten)]
    updates: CategoryUpdates,
}

impl UpdateCategoriesArgs {
    pub fn new<S, I>(names: I, updates: CategoryUpdates) -> Result<Self>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let names: Vec<String> = names.into_iter().map(|s| s.into()).collect();
        if names.is_empty() {
            return Err(anyhow!("At least one category name is required"))
                .pub_result(ErrorType::Request);
        }
        Ok(Self { names, updates })
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn updates(&self) -> &CategoryUpdates {
        &self.updates
    }
}

/// Args for the `tiller update autocats` command.
///
/// Updates one or more AutoCat rules in the local SQLite database by their IDs. At least one
/// ID must be provided. When more than one ID is provided, all specified rules are updated
/// with the same field values.
///
/// AutoCat rules have a synthetic auto-increment primary key that is assigned when first
/// synced down or inserted locally.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
#[derive(Debug, Parser, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateAutoCatsArgs {
    /// One or more AutoCat rule IDs to update. All specified rules will receive the same
    /// updates.
    #[arg(long, num_args = 1..)]
    ids: Vec<String>,

    /// The fields to update. Only fields with values will be modified; unspecified fields remain
    /// unchanged.
    #[clap(flatten)]
    updates: AutoCatUpdates,
}

impl UpdateAutoCatsArgs {
    pub fn new<S, I>(ids: I, updates: AutoCatUpdates) -> Result<Self>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let ids: Vec<String> = ids.into_iter().map(|s| s.into()).collect();
        if ids.is_empty() {
            return Err(anyhow!("At least one AutoCat ID is required"))
                .pub_result(ErrorType::Request);
        }
        Ok(Self { ids, updates })
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn updates(&self) -> &AutoCatUpdates {
        &self.updates
    }
}

// =============================================================================
// Delete command structs
// =============================================================================

/// Arguments for `tiller delete` commands.
#[derive(Debug, Parser, Clone)]
pub struct DeleteArgs {
    #[command(subcommand)]
    entity: DeleteSubcommand,
}

impl DeleteArgs {
    pub fn entity(&self) -> &DeleteSubcommand {
        &self.entity
    }
}

/// Subcommands for `tiller delete`.
#[derive(Subcommand, Debug, Clone)]
pub enum DeleteSubcommand {
    /// Deletes one or more transactions from the local SQLite database by their IDs. At least one
    /// transaction ID must be provided.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    ///
    /// **Warning**: This operation cannot be undone locally. However, if you haven't run `sync up`
    /// yet, you can restore the transactions by running `sync down` to re-download from the sheet.
    Transactions(DeleteTransactionsArgs),

    /// Deletes one or more categories from the local SQLite database by their names.
    ///
    /// Due to `ON DELETE RESTRICT` foreign key constraints, a category cannot be deleted if any
    /// transactions or AutoCat rules reference it. You must first update or delete those
    /// references before deleting the category.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    ///
    /// **Warning**: This operation cannot be undone locally. However, if you haven't run `sync up`
    /// yet, you can restore the categories by running `sync down` to re-download from the sheet.
    Categories(DeleteCategoriesArgs),

    /// Deletes one or more AutoCat rules from the local SQLite database by their IDs.
    ///
    /// AutoCat rules have synthetic auto-increment IDs assigned when first synced down or inserted
    /// locally.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    ///
    /// **Warning**: This operation cannot be undone locally. However, if you haven't run `sync up`
    /// yet, you can restore the rules by running `sync down` to re-download from the sheet.
    Autocats(DeleteAutoCatsArgs),
}

/// Args for the `tiller delete transactions` command.
///
/// Deletes one or more transactions from the local SQLite database by their IDs. At least one
/// transaction ID must be provided.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
///
/// **Warning**: This operation cannot be undone locally. However, if you haven't run `sync up`
/// yet, you can restore the transactions by running `sync down` to re-download from the sheet.
#[derive(Debug, Parser, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteTransactionsArgs {
    /// One or more transaction IDs to delete.
    #[arg(long = "id", required = true)]
    ids: Vec<String>,
}

impl DeleteTransactionsArgs {
    pub fn new<S, I>(ids: I) -> Result<Self>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let ids: Vec<String> = ids.into_iter().map(|s| s.into()).collect();
        if ids.is_empty() {
            return Err(anyhow!("At least one ID is required")).pub_result(ErrorType::Request);
        }
        Ok(Self { ids })
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

/// Args for the `tiller delete categories` command.
///
/// Deletes one or more categories from the local SQLite database by their names. At least one
/// category name must be provided.
///
/// Due to `ON DELETE RESTRICT` foreign key constraints, a category cannot be deleted if any
/// transactions or AutoCat rules reference it. You must first update or delete those references
/// before deleting the category.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
///
/// **Warning**: This operation cannot be undone locally. However, if you haven't run `sync up`
/// yet, you can restore the categories by running `sync down` to re-download from the sheet.
#[derive(Debug, Parser, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteCategoriesArgs {
    /// One or more category names to delete.
    #[arg(long = "name", required = true)]
    names: Vec<String>,
}

impl DeleteCategoriesArgs {
    pub fn new<S, I>(names: I) -> Result<Self>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let names: Vec<String> = names.into_iter().map(|s| s.into()).collect();
        if names.is_empty() {
            return Err(anyhow!("At least one category name is required"))
                .pub_result(ErrorType::Request);
        }
        Ok(Self { names })
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// Args for the `tiller delete autocats` command.
///
/// Deletes one or more AutoCat rules from the local SQLite database by their IDs. At least one
/// ID must be provided.
///
/// AutoCat rules have synthetic auto-increment IDs assigned when first synced down or inserted
/// locally.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
///
/// **Warning**: This operation cannot be undone locally. However, if you haven't run `sync up`
/// yet, you can restore the rules by running `sync down` to re-download from the sheet.
#[derive(Debug, Parser, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteAutoCatsArgs {
    /// One or more AutoCat rule IDs to delete.
    #[arg(long = "id", required = true)]
    ids: Vec<String>,
}

impl DeleteAutoCatsArgs {
    pub fn new<S, I>(ids: I) -> Result<Self>
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        let ids: Vec<String> = ids.into_iter().map(|s| s.into()).collect();
        if ids.is_empty() {
            return Err(anyhow!("At least one ID is required")).pub_result(ErrorType::Request);
        }
        Ok(Self { ids })
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

/// Arguments for `tiller insert` commands.
#[derive(Debug, Parser, Clone)]
pub struct InsertArgs {
    #[command(subcommand)]
    entity: InsertSubcommand,
}

impl InsertArgs {
    pub fn entity(&self) -> &InsertSubcommand {
        &self.entity
    }
}

/// Subcommands for `tiller insert`.
#[derive(Subcommand, Debug, Clone)]
pub enum InsertSubcommand {
    /// Inserts a new transaction into the local SQLite database.
    ///
    /// A unique transaction ID is automatically generated with a `user-` prefix to distinguish it
    /// from Tiller-created transactions. The generated ID is returned on success.
    ///
    /// The `date` and `amount` fields are required. All other fields are optional.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    Transaction(Box<InsertTransactionArgs>),

    /// Inserts a new category into the local SQLite database.
    ///
    /// The category name is required and must be unique as it serves as the primary key.
    /// The name is returned on success.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    Category(InsertCategoryArgs),

    /// Inserts a new AutoCat rule into the local SQLite database.
    ///
    /// AutoCat rules define automatic categorization criteria for transactions. The primary key
    /// is auto-generated and returned on success.
    ///
    /// All fields are optional - an empty rule can be created and updated later. However, a useful
    /// rule typically needs at least a category and one or more filter criteria.
    ///
    /// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
    Autocat(Box<InsertAutoCatArgs>),
}

/// Args for the `tiller insert transaction` command.
///
/// Inserts a new transaction into the local SQLite database. A unique transaction ID is
/// automatically generated with a `user-` prefix.
///
/// The `date` and `amount` fields are required. All other fields are optional.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
///
/// See tiller documentation for more information about the semantic meanings of transaction
/// columns: <https://help.tiller.com/en/articles/432681-transactions-sheet-columns>
#[derive(Debug, Clone, Parser, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "InsertTransactionArgs")]
pub struct InsertTransactionArgs {
    /// The posted date (when the transaction cleared) or transaction date (when the transaction
    /// occurred). Posted date takes priority except for investment accounts. **Required.**
    #[arg(long)]
    pub date: String,

    /// Transaction value where income and credits are positive; expenses and debits are negative.
    /// **Required.**
    #[arg(long)]
    pub amount: Amount,

    /// Cleaned-up merchant information from your bank.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description: Option<String>,

    /// The account name as it appears on your bank's website or your custom nickname from Tiller
    /// Console.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account: Option<String>,

    /// Last four digits of the bank account number (e.g., "xxxx1102").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account_number: Option<String>,

    /// Financial institution name (e.g., "Bank of America").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub institution: Option<String>,

    /// First day of the transaction's month, useful for pivot tables and reporting.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub month: Option<String>,

    /// Sunday date of the transaction's week for weekly breakdowns.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub week: Option<String>,

    /// Unmodified merchant details directly from your bank, including codes and numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub full_description: Option<String>,

    /// A unique ID assigned to your accounts by Tiller's systems. Important for troubleshooting;
    /// do not delete.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account_id: Option<String>,

    /// Check number when available for checks you write.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub check_number: Option<String>,

    /// When the transaction was added to the spreadsheet.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub date_added: Option<String>,

    /// Normalized merchant name standardizing variants (e.g., "Amazon" for multiple Amazon
    /// formats). Optional automated column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub merchant_name: Option<String>,

    /// Data provider's category suggestion based on merchant knowledge. Optional automated column;
    /// not included in core templates.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub category_hint: Option<String>,

    /// User-assigned category. Non-automated by default to promote spending awareness; AutoCat
    /// available for automation. Must reference an existing category name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub category: Option<String>,

    /// Custom notes about specific transactions. Leveraged by Category Rollup reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub note: Option<String>,

    /// User-defined tags for additional transaction categorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub tags: Option<String>,

    /// Date when AutoCat automatically categorized or updated a transaction. Google Sheets Add-on
    /// column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub categorized_date: Option<String>,

    /// For reconciling transactions to bank statements. Google Sheets Add-on column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub statement: Option<String>,

    /// Supports workflows including CSV imports. Google Sheets Add-on column.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub metadata: Option<String>,

    /// Custom columns not part of the standard Tiller schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[arg(long = "other-field", value_parser = utils::parse_key_val)]
    pub other_fields: BTreeMap<String, String>,
}

/// Args for the `tiller insert category` command.
///
/// Inserts a new category into the local SQLite database. The category name is required and
/// must be unique as it serves as the primary key.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
///
/// See tiller documentation for more information about the Categories sheet:
/// <https://help.tiller.com/en/articles/432680-categories-sheet>
#[derive(Debug, Clone, Parser, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "InsertCategoryArgs")]
pub struct InsertCategoryArgs {
    /// The name of the category. This is the primary key and must be unique. **Required.**
    #[arg(long)]
    pub name: String,

    /// The group this category belongs to. Groups organize related categories together for
    /// reporting purposes (e.g., "Food", "Transportation", "Housing"). All categories should have
    /// a Group assigned.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub group: Option<String>,

    /// The type classification for this category. Common types include "Expense", "Income", and
    /// "Transfer". All categories should have a Type assigned.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, name = "type")]
    pub r#type: Option<String>,

    /// Controls visibility in reports. Set to "Hide" to exclude this category from reports.
    /// This is useful for categories like credit card payments or internal transfers that you
    /// don't want appearing in spending reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub hide_from_reports: Option<String>,

    /// Custom columns not part of the standard Tiller schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[arg(long = "other-field", value_parser = utils::parse_key_val)]
    pub other_fields: BTreeMap<String, String>,
}

/// Args for the `tiller insert autocat` command.
///
/// Inserts a new AutoCat rule into the local SQLite database. The primary key is auto-generated
/// and returned on success.
///
/// All fields are optional - an empty rule can be created and updated later. However, a useful
/// rule typically needs at least a category and one or more filter criteria.
///
/// Changes are made locally only. Use `sync up` to upload local changes to the Google Sheet.
///
/// See tiller documentation for more information about AutoCat:
/// <https://help.tiller.com/en/articles/3792984-autocat-for-google-sheets>
#[derive(Debug, Clone, Parser, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "InsertAutoCatArgs")]
pub struct InsertAutoCatArgs {
    /// The category to assign when this rule matches. This is an override column - when filter
    /// conditions match, this category value gets applied to matching transactions. Must reference
    /// an existing category name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub category: Option<String>,

    /// Override column to standardize or clean up transaction descriptions. For example, replace
    /// "Seattle Starbucks store 1234" with simply "Starbucks".
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description: Option<String>,

    /// Filter criteria: searches the Description column for matching text (case-insensitive).
    /// Supports multiple keywords wrapped in quotes and separated by commas (OR-ed together).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description_contains: Option<String>,

    /// Filter criteria: searches the Account column for matching text to narrow rule application.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub account_contains: Option<String>,

    /// Filter criteria: searches the Institution column for matching text to narrow rule
    /// application.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub institution_contains: Option<String>,

    /// Filter criteria: minimum transaction amount (absolute value). Use with Amount Max to set
    /// a range. For negative amounts (expenses), set Amount Polarity to "Negative".
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, value_parser = utils::parse_amount)]
    pub amount_min: Option<Amount>,

    /// Filter criteria: maximum transaction amount (absolute value). Use with Amount Min to set
    /// a range. For negative amounts (expenses), set Amount Polarity to "Negative".
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, value_parser = utils::parse_amount)]
    pub amount_max: Option<Amount>,

    /// Filter criteria: exact amount to match.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long, value_parser = utils::parse_amount)]
    pub amount_equals: Option<Amount>,

    /// Filter criteria: exact match for the Description column (more specific than "contains").
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description_equals: Option<String>,

    /// Override column for the full/raw description field.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub description_full: Option<String>,

    /// Filter criteria: searches the Full Description column for matching text.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub full_description_contains: Option<String>,

    /// Filter criteria: searches the Amount column as text for matching patterns.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[arg(long)]
    pub amount_contains: Option<String>,

    /// Custom columns not part of the standard Tiller schema.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[arg(long = "other-field", value_parser = utils::parse_key_val)]
    pub other_fields: BTreeMap<String, String>,
}

fn default_tiller_home() -> DisplayPath {
    DisplayPath(match dirs::home_dir() {
        Some(home) => home.join("tiller"),
        None => {
            error!(
                "There was an error when trying to get your home directory. You can get around \
                this by providing --tiller-home or TILLER_HOME instead of relying on the default \
                tiller home directory. If you continue using the program right now, you may have \
                problems!",
            );
            PathBuf::from("tiller")
        }
    })
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct DisplayPath(PathBuf);

impl From<PathBuf> for DisplayPath {
    fn from(value: PathBuf) -> Self {
        DisplayPath(value)
    }
}

impl Deref for DisplayPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for DisplayPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Display for DisplayPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string_lossy())
    }
}

impl FromStr for DisplayPath {
    type Err = Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(PathBuf::from(s)))
    }
}

impl DisplayPath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}
