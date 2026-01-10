# Tiller Design

The tiller app provides two main modes of operation:

1. **CLI**: Command-line interface for syncing data between a local SQLite datastore and a Tiller
   Google Sheet.
2. **MCP**: Model Context Protocol server for AI agent integration. MCP tools wrap CLI commands,
   exposing the same functionality via JSON-RPC over stdio.

### Design Principles

- **Separation of Concerns**:
    - The `api` module handles OAuth and Google API operations.
    - The `commands` module contains top-level operations callable by both CLI and MCP.
    - The `model` module contains data-model structs such as `Transaction`.
    - The `mcp` module implements the MCP server, wrapping `commands` functions as tools.
- **Testability**: Google API operations use traits to enable mocking without requiring actual
  Google sheets interactions.

## CLI: High Level Overview

### Initialization: `tiller init`

Before using Tiller Sync, users must initialize their local directory structure. This is typically
the first command users run after setting up Google Cloud OAuth credentials.

```bash
# Initialize with default location ($HOME/tiller)
tiller init \
  --sheet-url "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID" \
  --client-secret ~/Downloads/client_secret_*.json

# Or specify a custom location
tiller init \
  --tiller-home /path/to/custom/location \
  --sheet-url "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID" \
  --client-secret ~/Downloads/client_secret_*.json
```

The `tiller init` command:

- Creates the data directory structure (including `.secrets/` and `.backups/` subdirectories)
- Moves the OAuth credentials file to `.secrets/client_secret.json`
- Creates an initial `config.json` with the provided settings (such as sheet URL)

**Arguments:**

- `--sheet-url`: URL of the user's Tiller Google Sheet (required)
- `--client-secret`: Path to the downloaded OAuth 2.0 client credentials file from Google Cloud
  Console (required)
- `--tiller-home`: Custom location for the tiller directory (optional, defaults to `$HOME/tiller`)

After running `tiller init`, users should run `tiller auth` to complete OAuth authentication.

### Authentication `tiller auth`

Once the user has set up the directory with `tiller init` they run the interactive command
`tiller auth`. This gives a Google URL at stdout that the user follows to authorize the OAuth scopes
we need. The app listens on http://localhost on a random port and accepts the OAuth token
information as a callback from the browser.

In detail, the workflow of `tiller auth` is as follows:

1. **Delete existing `token.json`** file if it exists.
2. **Load OAuth credentials** from `client_secret.json`
3. **Validate redirect URI** - Ensures the file contains the configured redirect URI
4. **Create OAuth client** using the `oauth2` crate
5. **Generate authorization URL** with required scope(s):
6. **Start local HTTP server** to receive the OAuth callback
7. **Open user's browser** to the authorization URL
8. **Wait for callback** - The local server captures the authorization code
9. **Request tokens**
10. **Save tokens** to `token.json`
11. **Shut down local server**
12. **Confirm success** to user

**Important**: `tiller auth` is the ONLY operation that initiates this interactive workflow. All
other operations should be scriptable.

#### `tiller auth --verify`

To check authentication, and refresh the token, users can call `tiller auth --verify`. The
`--verify` flag ensures that the command is non-interactive and will either

- Error if authentication does not work, or
- Report success if authentication worked and the tokens were refreshed

### Syncing

Uploading local changes to the Tiller Google Sheet:

```bash
# Upload and overwrite transactions and categories FROM the local datastore TO the Google Sheet
tiller sync up
```

Downloading changes from the Tiller Google Sheet:

```bash
# Download and overwrite transactions and categories FROM the Google Sheet TO the local datastore
tiller sync down
```

### MCP

The `tiller mcp` command launches an MCP server. See the dedicated MCP section later in this
document for details.

## Tiller Home: Local Directory Structure

There will be a local directory for storage and local editing of Tiller transactions and categories.
This directory is referred to as Tiller Home. The only command that works without a pre-existing
datastore directory is `tiller init`. Every other command will error out if the directory or config
file cannot be found.

A global flag will be needed to specify the location of this directory,

```
--tiller-home ~/my/location/for/tiller`.
```

However, it would be cumbersome to always provide this, so we will also accept it as an environment
variable:

```
TILLER_HOME
```

Finally, if it is not provided, it will by assumed to be a directory named `tiller` in the user's
home directory, where the home directory is determined in an OS-specific way. (This can be done with
the Rust `dirs` crate.)

The structure of the local directory will look like this:

```
├── .backups
│   ├── sync-down.2025-11-09-001.json
│   ├── tiller.sqlite.2025-11-08-001
│   ├── tiller.sqlite.2025-11-09-001
│   └── tiller.sqlite.2025-11-09-002
├── .secrets
│   ├── client_secret.json
│   └── token.json
├── config.json
└── tiller.sqlite
```

Each time a sync occurs, backups of the SQL Lite database and Google sheet are created.

## Configuration

```json
{
  "app_name": "tiller",
  "config_version": 1,
  "sheet_url": "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL",
  "backup_copies": 5,
  "client_secret_path": ".secrets/client_secret.json",
  "token_path": ".secrets/token.json"
}
```

**Note:** The `config_version` value is not intended to match the release version. It is an
independent identifier that can be used to handle breaking deserialization changes in the future.

The `client_secret_path` and `token_path` fields are optional. Paths can be absolute or relative to
the `config.json` file. If omitted, they default to `$TILLER_HOME/.secrets/client_secret.json` and
`$TILLER_HOME/.secrets/token.json` respectively.

## Datastore

The term *Local Datastore* or *Datastore* can either refer to the directory which contains all of
this, or to the main SQLite file, depending on context.

## Logging

Logging uses `tracing` and `tracing-subscriber`. All logging goes to `stderr`, leaving `stdout`
clean for CLI output and MCP protocol messages.

In MCP mode, important messages are also sent via MCP's `notifications/message` mechanism so the AI
client receives them. This dual approach (stderr + MCP notifications) allows debugging when running
`tiller mcp` manually while ensuring AI clients see relevant status information.

At the `info` level, query commands should be quiet. Sync operations can be more verbose since
their purpose is not to return data on `stdout`.

## Library Selection

The implementation uses:

- **`oauth2`** - OAuth 2.0 authentication flow implementation (provides full control over the auth
  process)
- **OxideComputer's `sheets` library** - Google Sheets API client library
- **`rmcp`** - Official Rust SDK for Model Context Protocol
- **`tracing`** - Structured logging and diagnostics

**Explicitly NOT using:**

- **`yup-oauth2`** - This library does not provide sufficient control over when and how the OAuth
  browser interaction occurs. We need explicit control to ensure only `tiller auth` can initiate
  interactive authentication.
- **`google-sheets4`** - This crate is tightly coupled to `yup-oauth2` and inherits the same
  limitations around authentication control.

**Why manual OAuth implementation:**

Historical Note: The problem with `yup-oauth2` is that it would automatically enter the interactive
workflow if there was any problem with authentication or missing scopes, etc. I could *not* find a
reasonable way to prevent this and I do not want this happening during CLI commands that are
expected to be non-interactive. Furthermore, the `google-sheets4` crate was deeply coupled to
`yup-oauth2`.

Thus: I decided to use `oauth2` and `sheets`.

### Credential Files

Two files are required for authentication, stored by default in `$TILLER_HOME/.secrets/`:

#### 1. `client_secret.json` - OAuth 2.0 Client Credentials

Users must obtain this from Google Cloud Console by creating OAuth 2.0 Desktop App credentials.
The file structure follows Google's standard format:

```json
{
  "installed": {
    "client_id": "YOUR_CLIENT_ID.apps.googleusercontent.com",
    "client_secret": "YOUR_CLIENT_SECRET",
    "redirect_uris": [
      "http://localhost"
    ],
    "auth_uri": "https://accounts.google.com/o/oauth2/auth",
    "token_uri": "https://oauth2.googleapis.com/token"
  }
}
```

The application will extract `client_id`, `client_secret`, and the first `redirect_uri` from this
file.

During the OAuth flow, the application automatically runs a temporary local HTTP server on a random
available port to capture the authorization callback from Google.

#### 2. `token.json` - Access and Refresh Tokens

Generated after successful OAuth consent flow. The file contains:

```json
{
  "scopes": [
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/drive.file"
  ],
  "access_token": "<ACCESS_TOKEN_DATA>",
  "refresh_token": "<REFRESH_TOKEN_DATA>",
  "expires_at": "2025-11-21T05:08:01.966146Z",
  "id_token": null
}
```

## Syncing Behavior

### Down

During the `tiller sync down` call, the following happens.

- If the datastore does not exist, it is created.
- A backup of the SQLite database is created.
- If more than `backup_copies` of the SQLite database exist, the extras are deleted.
- Three tabs from the `sheet_url`, *Transactions*, *Categories*, and *AutoCat*
- These are held in memory for further processing but also written out to
  `$TILLER_HOME/.backups/sync-down.2025-11-09-001.json`.
- If there are more than `backup_copies` of `sync-down.*.json` files, the oldest are deleted.
- Each of three tables in tiller.sqlite is upserted with the downloaded values.
    - Rows will be added to the database for new rows found in the sheets.
    - Rows will be deleted from the database for rows deleted from the sheets.
    - Rows will be updated in the database for rows that have been changed in the sheets.
    - Each row's `original_order` is set to its 0-indexed row position from the sheet.
- Cell formulas are captured and stored in the `formulas` table for potential preservation
  during sync up.

### Up

The `tiller sync up` command synchronizes local changes from the SQLite database to the Google
Sheet. This operation requires careful design to reduce accidental data corruption incidents.

#### Formula Preservation

Formulas in the original data present a challenge. Our behavior around preserving these is
necessarily somewhat complicated. Formula handling during `tiller sync up` is controlled by the
`--formulas` argument:

```
--formulas [unknown|preserve|ignore]
```

- **`unknown`** (default): If formulas exist in the database, error with a message instructing
  the user to explicitly choose `--formulas preserve` or `--formulas ignore`. If no formulas
  exist, proceed normally.
- **`preserve`**: Write formulas back to their original cell positions during sync up. This
  relies on `original_order` to maintain row alignment.
- **`ignore`**: Skip all formulas entirely; only write values.

**Deletion detection:** Gaps in the `original_order` sequence (e.g., 0, 1, 3) indicate deleted
rows. When deletions are detected, formula positions have shifted and they may not function
correctly when we put them back.

**Sync up behavior by `--formulas` value:**

| `--formulas` | Formulas Exist | Deletions Detected | Behavior                                      |
|--------------|----------------|--------------------|-----------------------------------------------|
| `unknown`    | No             | N/A                | Proceed normally                              |
| `unknown`    | Yes            | N/A                | ERROR: must specify `preserve` or `ignore`    |
| `ignore`     | Any            | Any                | Write values only, skip all formulas          |
| `preserve`   | Any            | No                 | Proceed, write formulas to original positions |
| `preserve`   | Any            | Yes (gaps)         | ERROR, require `--force` flag                 |
| `preserve`   | Any            | Yes + `--force`    | Write formulas to original positions (forced) |

#### Safety Measures

1. **Always backup before syncing** - Backup the data to `sync-up-pre.YYYY-MM-DD-NNN.json` and also
   clone the Google sheet.
2. **Conflict detection** - Warn if sheet was modified since last download
3. **Explicit formula handling** - A `--formulas` argument is required to specify formula
   preservation behavior
4. **`--force` flag** - Required to overwrite Google sheet in the presence of detected conflicts or
   formulas that may be corrupted
5. **Consistent column order** - Always write headers explicitly to control column positions
6. **Verification** - Confirm write succeeded by checking row counts
7. **Comprehensive logging** - All operations logged to stderr for debugging

#### Strategy: Clear and Replace with Verification

Given these constraints, the safest approach is to treat the local SQLite database as the
authoritative source of truth and completely replace the sheet contents. This strategy eliminates
dependencies on row/column ordering and provides predictable, repeatable results.

**Algorithm:**

1. **Precondition Checks**
    - a. If the datastore does not exist, error with message: "Run `tiller sync down` first"
    - b. If the SQLite database is empty of transactions, error with message: "Run `tiller sync
      down` first"

2. **Download Current Sheet State and back it up**
    - a. Fetch all three tabs: Transactions, Categories, AutoCat
    - b. Save to backup file: `$TILLER_HOME/.backups/sync-up-pre.YYYY-MM-DD-NNN.json`
    - c. This serves as a snapshot of what we're about to overwrite
    - d. Delete the oldest `sync-up-pre` snapshot if more than `backup_copies` exist

3. **Conflict Detection**
    - a. Find most recent `sync-down.*.json` backup
    - b. If no backup exists and `--force` not provided:
        - Error: "No sync-down backup found. Run 'tiller sync down' first, or use --force to
          proceed without conflict detection"
    - c. If no backup exists and `--force` provided: skip conflict detection, proceed
    - d. Compare downloaded sheet data with the backup
    - e. If differences detected, warn user: "Sheet has been modified since last sync down"
    - f. Count differences: `N transactions added, M modified, P deleted since last download`
    - g. Recommend: "Merge changes manually and run 'tiller sync down' first, or use --force to
      overwrite"
    - h. If `--force` not provided, abort sync

4. **Build Output Data**
    - a. For each tab (Transactions, Categories, AutoCat):
        - Query all rows from corresponding SQLite table
        - Build header row from `sheet_metadata` table (preserves original column order and names)
        - Build data rows in consistent column order matching the headers
        - Ensure calculated fields are populated (Month, Week for transactions)
        - Sort rows by `original_order ASC NULLS LAST`, then by primary key for determinism
        - Locally-added rows (NULL `original_order`) are appended at the end
    - b. When deserializing to `TillerData`, always query formulas from the `formulas` table and
      include them in the deserialized data, regardless of `--formulas` mode. Build a map of
      (row, col) -> formula. The `--formulas` flag determines what we do with these formulas
      in subsequent steps, not whether we load them.

5. **Formula Safety Checks**: Having already deserialized to `TillerData`, the following steps occur
   in the `model` and `commands` layers.
    - a. If `--formulas unknown` (default) and formulas exist:
        - Error: "Formulas detected in database. Use `--formulas preserve` or `--formulas ignore`"
    - b. If `--formulas preserve`:
        - Run gap detection on `original_order` of the data now held in `TillerData`.
        - If gaps detected and `--force` not provided:
            - Error: "Row deletions detected. Formula positions may be corrupted. Use `--force` to
              proceed anyway, or use `--formulas ignore`"
        - If gaps detected and `--force` is provided, use `warn!` to make note of this and
          proceed.
    - c. If `--formulas ignore`: proceed without formula handling

6. **Backup SQLite**
    - a. Create backup of SQLite database with timestamp (e.g., `tiller.sqlite.2025-11-21-003`)
    - b. Delete the oldest backups if more than `backup_copies` exist

7. **Backup Google Sheet**
    - a. Use the Google Drive API `files.copy` endpoint to create a full copy of the spreadsheet
    - b. Set the copy's name to `<original-sheet-name>-backup-YYYY-MM-DD-NNN`
    - c. This requires the `drive.file` scope
    - d. Store the backup file ID in the sync log for potential recovery
    - e. Consider: delete old backup copies from Drive if more than `backup_copies` exist

8. **Execute Batch Clear and Write**
    - a. Use `spreadsheets().values_batch_clear()` to clear, then `values_batch_update()` to write
    - b. All write operations use `ValueInputOption::UserEntered` to allow Sheets to parse
      dates, numbers, and formulas
    - c. **Clear**: Clear each tab entirely (headers and data)
        - Transactions: `"Transactions!A1:ZZ"`
        - Categories: `"Categories!A1:ZZ"`
        - AutoCat: `"AutoCat!A1:ZZ"`
    - d. **Write**: Write all rows (headers + data) in a single operation
        - Transactions: `"Transactions!A1:ZZ"`
        - Categories: `"Categories!A1:ZZ"`
        - AutoCat: `"AutoCat!A1:ZZ"`
    - e. **Write formulas** (only if `--formulas preserve`): Write formulas to original positions
        - For each formula in the map, write to cell at (row + 2, col + 1) in A1 notation
        - Row offset of 2 accounts for 1-indexed sheets plus header row

9. **Verification**
    - a. Re-fetch row counts from each tab
    - b. Verify counts match what we wrote
    - c. Log summary: `"Synced N transactions, M categories, P autocat rules to sheet"`
    - d. If `--formulas preserve`: log count of formulas written per sheet

10. **Error Handling**
    - a. If any operation fails, the backup files allow manual recovery
    - b. Log all operations to stderr at INFO level
    - c. On failure, provide clear message about which backup to restore and hint at how to do it.

#### Future Enhancements

1. **Three-way merge**: Track "last synced state" to detect conflicts in both local and remote
2. **Timestamp tracking**: Add `last_modified_local` field to detect which rows changed locally
3. **Selective sync**: Only upload transactions modified since last sync
4. **Transaction-level conflict resolution**: Merge changes at the row level when possible

#### User Workflow

**Safe workflow** (recommended):

```bash
# 1. Download latest changes from sheet
tiller sync down

# 2. Make local edits in SQLite

# 3. Upload local changes back to sheet
tiller sync up
```

**Forcing overwrites** (when local is authoritative):

```bash
# Force upload despite remote changes
tiller sync up --force
```

### Row IDs

#### Transaction IDs

For the *Transactions* tab, we will use the `Transaction ID` column as our primary key in the
database. When this column is populated with an ID that looks like the following:

```text
69112cec0a57f52108456b88
690edd882cac40d381f9e518
690edd882cac40d381f9e519
690edd882cac40d381f9e51a
690edd882cac40d381f9e51b
```

Then this is an ID created by, and provided by Tiller.

When we need to create our own IDs because we are adding rows, then they will look like this:

```text
user-f47e8c2a9b3d4f1ea80
```

This is a UUIDv4 with the dashes removed, 13-characters removed at random, and prepended with
`user-`.

We will represent this with a Rust enum like this:

```rust
enum IdType {
    Tiller,
    Local,
}

struct TransactionId {
    value: String,
    id_type: IdType,
}
```

- We will implement a `Default` function for this using the `uuid` crate that creates a `Local` ID.
- We will implement Serialize and Deserialize

## Database Schema

The SQLite database contains the following tables. See `src/db/migrations/` for exact DDL.

### Data Tables

**transactions** - Financial transactions from the Tiller Transactions sheet.

- Primary key: `transaction_id` (Tiller-assigned or `user-` prefixed local UUID)
- Indexed on: `date`, `account`, `category`, `description`
- Foreign key: `category` references `categories(category)` with `ON UPDATE CASCADE ON DELETE
  RESTRICT`

**categories** - Budget categories from the Tiller Categories sheet.

- Primary key: `category` (the category name itself)

**autocat** - Automatic categorization rules from the Tiller AutoCat sheet.

- Primary key: `id` (synthetic auto-increment)
- Foreign key: `category` references `categories(category)` with `ON UPDATE CASCADE ON DELETE
  RESTRICT`

All three data tables include:

- `original_order INTEGER` - Row position from last sync down (0-indexed); NULL for locally-added
  rows. Used for formula preservation.
- `other_fields TEXT` - JSON object storing unknown/custom columns keyed by original header name.

### Foreign Key Semantics

The foreign key constraints enforce referential integrity between transactions/autocat and
categories:

- **ON UPDATE CASCADE**: When a category is renamed, all transactions and autocat rules referencing
  that category are automatically updated to use the new name.
- **ON DELETE RESTRICT**: A category cannot be deleted if any transactions or autocat rules
  reference it. Those references must be updated or removed first.

**Bulk sync operations**: During `sync down` and `sync up`, foreign key constraints are temporarily
disabled using `PRAGMA foreign_keys = OFF` to allow the efficient delete-all-then-insert pattern for
categories and autocat. The data from Tiller's Google Sheet is assumed to be internally consistent.

### Metadata Tables

**sheet_metadata** - Column ordering and header mapping per sheet.

- Primary key: `(sheet, "order")`
- Unique: `(sheet, header_name)`
- Stores all columns including custom ones, enabling round-trip preservation of sheet structure.

**formulas** - Cell formulas from the Google Sheet.

- Primary key: `(sheet, row, col)`
- Formulas are tied to sheet positions, not row data. See Formula Preservation.

## Schema Migrations

The SQLite database uses a version-based migration system to manage schema changes over time.

### Version Tracking

A `schema_version` table tracks the current database schema version:

```sql
CREATE TABLE schema_version
(
    version INTEGER NOT NULL
);
```

This table contains a single row with the current schema version number. When a new database is
created, this table is bootstrapped in Rust code with version `0`. This bootstrap step is separate
from the migration system - it establishes the invariant that `schema_version` always exists,
allowing the migration logic to work uniformly for all migrations including `migration_01`.

### Migration Files

Migration files are stored in `src/db/migrations/` with the naming convention:

- `migration_NN_up.sql` - Upgrades schema from version `NN-1` to version `NN`
- `migration_NN_down.sql` - Downgrades schema from version `NN` to version `NN-1`

For example:

- `migration_01_up.sql` brings the schema from version 0 to version 1
- `migration_01_down.sql` reverts from version 1 back to version 0
- `migration_02_up.sql` brings the schema from version 1 to version 2

Migration files are embedded into the binary at compile time using `include_str!`. Each migration
file must be manually added to the list of embedded files in the source code.

### Version Constant

A `CURRENT_VERSION` constant in `src/db/mod.rs` defines the target schema version. This equals the
highest migration number available. For example, if `migration_05_up.sql` is the highest numbered
migration, `CURRENT_VERSION` is `5`.

### Migration Execution

When the database is loaded:

1. Query the current version: `SELECT MAX(version) FROM schema_version`
2. Compare against `CURRENT_VERSION`
3. If the database version is lower, run "up" migrations sequentially to reach `CURRENT_VERSION`
4. If the database version is higher, run "down" migrations sequentially to reach `CURRENT_VERSION`

Each migration is executed within a SQLite transaction (managed in Rust code). The transaction
includes both the migration SQL and the update to `schema_version`, ensuring they succeed or fail
together.

### Error Handling

If a migration fails:

- The failed migration's transaction is rolled back
- The database remains at the last successfully applied version
- An error is returned with details about which migration failed

For example, if migrating from version 0 to version 5 and migration 3 fails, the database remains at
version 2 (after migrations 1 and 2 succeeded).

### Logging

Migration activity is logged at `debug` level:

- `"Running migration 01 (up)"`
- `"Running migration 03 (down)"`
- `"Migration complete, schema now at version 3"`

### Implementation Details

The `Db` struct provides a private method to query the current schema version:

```rust
async fn schema_version(&self) -> Result<i32> {
    // SELECT MAX(version) FROM schema_version
}
```

The `Db::load()` and `Db::init()` methods handle migration execution:

- `Db::init()` - Creates the database file, bootstraps `schema_version` with version 0, then runs
  migrations to reach `CURRENT_VERSION`
- `Db::load()` - Opens an existing database and runs any needed migrations (up or down) to reach
  `CURRENT_VERSION`

## Db Data Interface

The `Db` struct exposes `save_tiller_data` and `get_tiller_data` methods for syncing data
with the database.

**Sync semantics for `save_tiller_data`:**

- **Transactions**: Upsert - insert new, update existing, delete rows not in incoming data
- **Categories**: Delete all, then insert all
- **AutoCat**: Delete all, then insert all

All operations run within a single SQLite transaction with rollback on error.

## MCP Server

The `tiller mcp` command runs an MCP (Model Context Protocol) server, enabling AI agents to interact
with Tiller data programmatically.

### Transport

Uses stdio transport: the MCP client launches `tiller mcp` as a subprocess and communicates via
JSON-RPC over stdin/stdout.

### Configuration

The `--tiller-home` flag and `TILLER_HOME` environment variable work identically to other commands.

### SDK

Uses the official `rmcp` crate for MCP protocol implementation.

### Tools

MCP tools wrap CLI commands with equivalent parameters:

- **sync_down**: Downloads data from Google Sheet to local SQLite. No parameters.
- **sync_up**: Uploads data from local SQLite to Google Sheet. Parameters: `force` (bool),
  `formulas` (enum: unknown, preserve, ignore).

### Tool Responses

For `sync up` and `sync down` tools we return minimal responses: success/failure status and a
summary message (e.g., "Synced 1,234 transactions, 45 categories, 12 autocat rules"). Full data is
not included in responses when syncing. Full data *is* included for queries (e.g. select
statements).

### Error Handling

Tool execution failures use the MCP `isError` pattern: return a successful JSON-RPC response with
`is_error: true` and a descriptive message. Protocol-level errors use standard JSON-RPC error codes.

The application error type implements conversion to both `rmcp::ErrorData` (for protocol errors) and
provides a helper for creating `CallToolResult::error()` responses (for tool failures).

### Logging

Dual logging approach:

- **stderr**: All log output via `tracing`, useful for debugging when running `tiller mcp` manually.
- **MCP notifications**: Important messages sent via `notifications/message` so AI clients see them.

## Query Interface

The query interface provides AI agents and CLI users the ability to query locally stored data using
raw SQL. This is designed primarily for AI agents via MCP, with CLI as a secondary interface.

### Design Decisions

1. **Raw SQL**: AI agents are excellent at generating SQL, and raw SQL provides maximum flexibility.
   No structured query parameters or predefined queries - just pass SQL strings directly.

2. **Read-Only Access**: The query interface enforces read-only access using a separate SQLite
   connection opened with `?mode=ro`. This is bulletproof - SQLite rejects any write attempt at the
   database level. Mutations must use the existing CRUD tools (`update_transactions`,
   `insert_transaction`, etc.).

3. **Dual Connection Pool**: The `Db` struct maintains two connection pools:
    - Read-write pool: Used by CRUD operations and sync commands
    - Read-only pool: Used exclusively by the `query` tool

4. **No Row Limits**: The interface does not enforce row limits. AI agents are trusted to use
   `LIMIT` clauses appropriately. Documentation warns that result sets can be large.

5. **Output Formats**: Both CLI and MCP support three output formats via the `format` parameter:
    - `json` (default for CLI, required for MCP): JSON array of objects where each row is a
      self-describing object with column names as keys
    - `markdown`: Markdown table format
    - `csv`: CSV format

6. **Error Handling**: SQL errors are wrapped with `.context("SQL error")` to provide the SQLite
   error message with a clear prefix.

### CLI Commands

```bash
# Execute a SQL query (defaults to JSON output)
tiller query "SELECT * FROM transactions WHERE category = 'Food' LIMIT 10"

# Specify output format
tiller query --format markdown "SELECT * FROM transactions LIMIT 5"
tiller query --format csv "SELECT category, SUM(amount) FROM transactions GROUP BY category"

# Get database schema (data tables only)
tiller schema

# Include metadata tables in schema
tiller schema --include-metadata
```

### MCP Tools

Two new MCP tools wrap the CLI commands:

- **query**: Executes arbitrary read-only SQL. Parameters: `sql` (required), `format` (required).
- **schema**: Returns database schema information. Parameters: `include_metadata` (optional,
  defaults to false).

### Query Tool

**Parameters:**

- `sql: String` - The SQL query to execute (required)
- `format: OutputFormat` - Output format: `json`, `markdown`, or `csv` (required for MCP, defaults
  to `json` for CLI)

**Returns:** `Out<Rows>` where `Rows` is an enum:

```rust
pub enum Rows {
    Json(serde_json::Value),  // Array of objects
    Table(String),            // Markdown table as a formatted string
    Csv(String),              // CSV data with proper escaping
}
```

The `Out.message` states the number of rows returned (e.g., "Query returned 42 rows").

### Schema Tool

**Parameters:**

- `include_metadata: bool` - Whether to include internal tables (`sheet_metadata`, `formulas`,
  `schema_version`). Defaults to `false`.

**Returns:** `Out<Schema>` - always returns structured data (no format parameter).

```rust
pub struct Schema {
    pub tables: Vec<TableInfo>,
}

pub struct TableInfo {
    pub name: String,
    pub row_count: u64,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
}

pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub description: Option<String>,
}

pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

pub struct ForeignKeyInfo {
    pub columns: Vec<String>,
    pub references_table: String,
    pub references_columns: Vec<String>,
}
```

### Column Descriptions

Column descriptions in `Schema` output come from doc comments on model struct fields via
`schemars`. The `Item` trait has a `JsonSchema` supertrait bound and provides a default
implementation of `field_descriptions() -> BTreeMap<String, String>` that extracts descriptions
from the JsonSchema at runtime. This provides a single source of truth - descriptions are
maintained only in model doc comments.

## Notes

This describes editing the transactions sheet:
https://help.tiller.com/en/articles/432679-editing-the-transactions-sheet

This is very important documentation describing each column and what it is for:
https://help.tiller.com/en/articles/432681-transactions-sheet-columns
