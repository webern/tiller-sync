# Tiller Design

The tiller app provides two main modes of operation:

1. CLI based: for syncing data between a local datastore and the user's Tiller Google sheet from the
   command line
2. MCP interface: so that it can be used by an AI agent.

### Design Principles

- **Separation of Concerns**:
    - The `api` module code focuses on OAuth and Google API operations
    - The `commands` module has top-level, end-to-end operations that can be called by either the
      CLI mode or the MCP mode.
    - The `model` module contains data-model structs such as `Transaction`.
- **Testability**: Google API operations use traits to enable mocking without requiring actual
  Google sheets interactions

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

The `tiller mcp` command runs as a long-running process that communicates via JSON-RPC over
stdin/stdout, implementing the Model Context Protocol. MCP clients (like Claude Code) launch
`tiller mcp` as a subprocess and send JSON-RPC requests on stdin, receiving responses on stdout.
The MCP interface shares the same underlying business logic as the CLI commands.

**Running MCP mode:**

```bash
# MCP client launches this as a subprocess and communicates via stdin/stdout
tiller mcp
```

### CLI and MCP Agreement

In general, there will be an agreement between the CLI interface a person can use, and the tools
that are available in MCP mode.

For example, the following (theoretical) query command could also be made available as an MCP tool:

```bash
tiller query transactions \
  --category Groceries \
  --start-date '2024-10-01' \
  --end-date '2024-10-31'
```

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

Logging will be achieved with `log` and `env_logger`. All logging will be sent to `stderr`. Leaving
`stdout` for clean output both in MCP and CLI modes. At the `info` logging level, commands such as
`tiller query` should be extremely quiet, preferably silent. However for `tiller sync` operations,
`info` logging can be more robust since the call is not about receiving anything on `stdout`.

In other words, commands whose purpose is to send data to `stdout` should be quiet at the `info`
logging level so that users who use `2>&1` under normal circumstances won't have a problem.

## Library Selection

The implementation uses:

- **`oauth2`** - OAuth 2.0 authentication flow implementation (provides full control over the auth
  process)
- **OxideComputer's `sheets` library** - Google Sheets API client library

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
5. **`--dry-run` flag** - Preview what would be changed without actually syncing
6. **Consistent column order** - Always write headers explicitly to control column positions
7. **Verification** - Confirm write succeeded by checking row counts
8. **Comprehensive logging** - All operations logged to stderr for debugging

#### Strategy: Clear and Replace with Verification

Given these constraints, the safest approach is to treat the local SQLite database as the
authoritative source of truth and completely replace the sheet contents. This strategy eliminates
dependencies on row/column ordering and provides predictable, repeatable results.

**Algorithm:**

1. **Precondition Checks**
    - If the datastore does not exist, error with message: "Run `tiller sync down` first"
    - If the SQLite database is empty of transactions, error with message: "Run `tiller sync down`
      first"

2. **Backup SQLite**
    - Create backup of SQLite database with timestamp (e.g., `tiller.sqlite.2025-11-21-003`)
    - Delete the oldest backups if more than `backup_copies` exist

3. **Download Current Sheet State**
    - Fetch all three tabs: Transactions, Categories, AutoCat
    - Save to backup file: `$TILLER_HOME/.backups/sync-up-pre.YYYY-MM-DD-NNN.json`
    - This serves as a snapshot of what we're about to overwrite
    - Delete the oldest `sync-up-pre` snapshot if more than `backup_copies` exist

4. **Conflict Detection**
    - Compare downloaded sheet data with most recent `sync-down.*.json` backup
    - If differences detected, warn user: "Sheet has been modified since last sync down"
    - Count differences: `N transactions added, M modified, P deleted since last download`
    - Recommend: "Merge changes manually and run 'tiller sync down' first, or use --force to
      overwrite"
    - If `--force` not provided, abort sync

5. **Formula Safety Checks**
    - Query the `formulas` table to check if any formulas exist
    - If `--formulas unknown` (default) and formulas exist:
        - Error: "Formulas detected in database. Use `--formulas preserve` or `--formulas ignore`"
    - If `--formulas preserve`:
        - Run gap detection on `original_order` (see step 5a)
        - If gaps detected and `--force` not provided:
            - Error: "Row deletions detected. Formula positions may be corrupted. Use `--force` to
              proceed anyway, or use `--formulas ignore`"
    - If `--formulas ignore`: proceed without formula handling

    **Step 5a: Gap Detection Algorithm**
    - For each sheet (transactions, categories, autocat):
        - Query: `SELECT original_order FROM {table} WHERE original_order IS NOT NULL ORDER BY
          original_order ASC`
        - Iterate through results expecting sequential values: 0, 1, 2, 3...
        - If any gap exists (e.g., 0, 1, 3), record that deletions occurred for this sheet
    - Return the set of sheets with detected deletions

6. **Build Output Data**
    - For each tab (Transactions, Categories, AutoCat):
        - Query all rows from corresponding SQLite table
        - Build header row from `sheet_metadata` table (preserves original column order and names)
        - Build data rows in consistent column order matching the headers
        - Ensure calculated fields are populated (Month, Week for transactions)
        - Sort rows by `original_order ASC NULLS LAST`, then by primary key for determinism
        - Locally-added rows (NULL `original_order`) are appended at the end
    - If `--formulas preserve`:
        - Query formulas from `formulas` table for each sheet
        - Build a map of (row, col) -> formula for use during write

7. **Backup Google Sheet**
    - Use the Google Drive API `files.copy` endpoint to create a full copy of the spreadsheet
    - Set the copy's name to `<original-sheet-name>-backup-YYYY-MM-DD-NNN`
    - This requires the `drive.file` scope
    - Store the backup file ID in the sync log for potential recovery
    - Consider: delete old backup copies from Drive if more than `backup_copies` exist

8. **Execute Batch Clear and Write**
    - Using `spreadsheets().values_batch_update()` for efficiency:
        - **Operation 1**: Clear each tab's data range (preserve sheet structure, delete all rows
          except header)
            - Transactions: `"Transactions!A2:ZZ"` (everything below header row)
            - Categories: `"Categories!A2:ZZ"`
            - AutoCat: `"AutoCat!A2:ZZ"`
        - **Operation 2**: Write header rows
            - Transactions: `"Transactions!A1:ZZ1"`
            - Categories: `"Categories!A1:ZZ1"`
            - AutoCat: `"AutoCat!A1:ZZ1"`
        - **Operation 3**: Write all data rows (values only)
            - Transactions: `"Transactions!A2:ZZ"` (dynamic based on row count)
            - Categories: `"Categories!A2:ZZ"`
            - AutoCat: `"AutoCat!A2:ZZ"`
        - **Operation 4** (only if `--formulas preserve`): Write formulas to original positions
            - For each formula in the map, write to cell at (row + 2, col + 1) in A1 notation
            - Row offset of 2 accounts for 1-indexed sheets plus header row
            - Use `ValueInputOption::UserEntered` so formulas are interpreted
    - Use `ValueInputOption::UserEntered` to allow Sheets to parse dates/numbers/formulas

9. **Verification**
    - Re-fetch row counts from each tab
    - Verify counts match what we wrote
    - Log summary: `"Synced N transactions, M categories, P autocat rules to sheet"`
    - If `--formulas preserve`: log count of formulas written per sheet

10. **Error Handling**
    - If any operation fails, the backup files allow manual recovery
    - Log all operations to stderr at INFO level
    - On failure, provide clear message about which backup to restore and hint at how to do it.

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
# Preview changes
tiller sync up --dry-run

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

**categories** - Budget categories from the Tiller Categories sheet.

- Primary key: `id` (synthetic auto-increment)
- Unique constraint on `category` (allows renaming categories)

**autocat** - Automatic categorization rules from the Tiller AutoCat sheet.

- Primary key: `id` (synthetic auto-increment)

All three data tables include:

- `original_order INTEGER` - Row position from last sync down (0-indexed); NULL for locally-added
  rows. Used for formula preservation.
- `other_fields TEXT` - JSON object storing unknown/custom columns keyed by original header name.

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

## Notes and Todos

This describes editing the transactions sheet:
https://help.tiller.com/en/articles/432679-editing-the-transactions-sheet

This is very important documentation describing each column and what it is for:
https://help.tiller.com/en/articles/432681-transactions-sheet-columns
