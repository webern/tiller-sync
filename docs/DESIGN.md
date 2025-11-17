# Tiller Design

The tiller app provides two main modes of operation, one for syncing data between a local datastore
and the user's Tiller Google sheet, and one that provides the MCP interface so that it can be used
as an MCP tool.

## Project Structure

The codebase follows a modular organization with clear separation of concerns:

### Module Organization

- **`src/api/`** - Google Sheets and OAuth operations
    - Contains all Google API interactions, OAuth flow, and API client wrappers
    - Defines traits for API operations to enable mocking in tests
    - **Does NOT** contain file path resolution logic

- **`src/config.rs`** - Configuration file handling
    - Manages loading/saving `config.json`
    - Contains helper functions for resolving credential file paths (api_key.json, token.json)
    - Handles logic for default paths vs. config-specified paths (relative or absolute)

- **`src/utils.rs`** - Reusable utility functions
    - General-purpose utilities used across the codebase
    - File I/O helpers and other common operations

- **`src/args.rs`** - CLI argument parsing
    - Clap structures for command-line interface

- **`src/home.rs`** - Home directory management
    - Handles `TILLER_HOME` directory operations

### Design Principles

1. **Separation of Concerns**: API code focuses on API operations; configuration code handles paths
   and settings
2. **Testability**: API operations use traits to enable mocking without requiring actual Google
   credentials
3. **Reusability**: Common utilities belong in `utils.rs` for use across modules

## Interface: High Level Overview

### Initialization

Before using Tiller Sync, users must initialize their local directory structure. This is typically
the first command users run after setting up Google Cloud OAuth credentials.

```bash
# Initialize with default location ($HOME/tiller)
tiller init \
  --sheet-url "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID" \
  --api-key ~/Downloads/client_secret_*.json

# Or specify a custom location
tiller init \
  --tiller-home /path/to/custom/location \
  --sheet-url "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID" \
  --api-key ~/Downloads/client_secret_*.json
```

The `tiller init` command:

- Creates the data directory structure (including `.secrets/` and `.backups/` subdirectories)
- **Moves** the OAuth credentials file to `.secrets/api_key.json`
- Creates an initial `config.json` with the provided sheet URL and default settings

**Arguments:**

- `--sheet-url`: URL of the user's Tiller Google Sheet (required)
- `--api-key`: Path to the downloaded OAuth credentials file from Google Cloud Console (required)
- `--tiller-home`: Custom location for the tiller directory (optional, defaults to `$HOME/tiller`)

After running `tiller init`, users should run `tiller auth` to complete OAuth authentication.

### Syncing

Uploading local changes to the Tiller Google Sheet:

```bash
# Upload transactions and categories from the local datastore, overwriting where different
tiller sync up
```

Downloading changes from the Tiller Google Sheet:

```bash
# Download transactions and categories to the local datastore, overwriting where difference
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

**Theoretical interaction:**

Input (JSON-RPC request from MCP client via stdin):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "tiller__query_transactions",
    "arguments": {
      "category": "Groceries",
      "start_date": "2024-10-01",
      "end_date": "2024-10-31"
    }
  }
}
```

Output (JSON-RPC response to MCP client via stdout):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "transactions": [
      {
        "id": "tx_123",
        "date": "2024-10-15",
        "description": "Whole Foods",
        "amount": -87.43,
        "category": "Groceries",
        "account": "Checking"
      },
      {
        "id": "tx_124",
        "date": "2024-10-22",
        "description": "Trader Joes",
        "amount": -65.20,
        "category": "Groceries",
        "account": "Checking"
      }
    ],
    "total": -152.63,
    "count": 2
  }
}
```

### CLI and MCP Agreement

In general, there will be an agreement between the CLI interface a person can use, and the tools
that are available in MCP mode.

For example: The above theoretical interaction would also be available directly as something
like this:

```bash
tiller query transactions \
  --category Groceries \
  --start-date '2024-10-01' \
  --end-date '2024-10-31'
```

## Local Directory Structure

There will be a local directory for storage and local editing of Tiller transactions and categories.

A global flag will be needed to specify the location of this directory,

```
--dir ~/my/location/for/tiller`.
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
│   ├── download.2025-11-09-001.json
│   ├── tiller.sqlite.2025-11-08-001
│   ├── tiller.sqlite.2025-11-09-001
│   └── tiller.sqlite.2025-11-09-002
├── .secrets
│   ├── api_key.json
│   └── token.json
├── config.json
└── tiller.sqlite
```

Each time a sync occurs, a backup of the SQLite database is created. The backup is a simple copy of
the SQLite database file. The number of copies of backups to keep is configurable. A basic
configuration file looks like this:

```json
{
  "app_name": "tiller",
  "config_version": 1,
  "sheet_url": "https://docs.google.com/spreadsheets/d/7KpXm2RfZwNJgs84QhVYno5DU6iM9Wlr3bCzAv1txRpL",
  "backup_copies": 5,
  "api_key_path": ".secrets/api_key.json",
  "token_path": ".secrets/token.json"
}
```

**Note:** The `config_version` value is not intended to match the release version. It is an
independent identifier that can be used to handle breaking deserialization changes in the future.

The `api_key_path` and `token_path` fields are optional. Paths can be absolute or relative to
the `config.json` file. If omitted, they default to `$TILLER_HOME/.secrets/api_key.json` and
`$TILLER_HOME/.secrets/token.json` respectively.

The term "Local Datastore" can either refer to the directory which contains all of this, or to the
main SQLite file, depending on context.

When the app starts, if the datastore directory does not exist, it will be created. If the datastore
directory exists, but it does not conform to the expected structure and naming conventions, an error
will be raised.

## Logging

Logging will be achieved with `log` and `env_logger`. All logging will be sent to `stderr`. Leaving
`stdout` for clean output both in MCP and CLI modes. At the `info` logging level, commands such as
`tiller query` should be extremely quiet, preferably silent. However for `tiller sync` operations,
`info` logging can be more robust since the call is not about receiving anything on `stdout`.

In other words, commands whose purpose is to send data to `stdout` should be quiet at the `info`
logging level so that users who use `2>&1` under normal circumstances won't have a prolem.

## Google Sheets Authentication

Google Sheets API access requires OAuth 2.0 credentials. The authentication workflow consists of
an initial setup phase where users obtain credentials and complete OAuth consent, followed by
automatic token management for ongoing operations.

The implementation uses:

- **`yup-oauth2`** - OAuth 2.0 authentication and token management
- **`google-sheets4`** - Google Sheets API client library

These libraries handle most of the OAuth complexity automatically, including:

- Local HTTP server for OAuth callback (on port 3030)
- Token persistence to disk
- Automatic token refresh when expired
- Browser opening for authorization

### Code Organization

All Google Sheets and Google Auth related code will be located in `src/api/`. This includes:

- OAuth authentication flow implementation (`src/api/oauth.rs`)
- Google Sheets client wrapper (`src/api/sheets_client.rs`)
- Credential file structures (`src/api/files.rs`)
- Token management (handled by `yup-oauth2`)

To enable testing, we will wrap sheets and OAuth operations in a trait that can be mocked and
injected. The production code will use implementations backed by `yup-oauth2` and `google-sheets4`,
while tests will use mock implementations.

### Credential Files

Two files are required for authentication, stored by default in `$TILLER_HOME/.secrets/`:

#### 1. `api_key.json` - OAuth 2.0 Client Credentials

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

**Important**: When creating OAuth credentials in Google Cloud Console, users must set the redirect
URI to `http://localhost:3030`. This URI must match exactly what is configured in the Google Cloud
Console and what appears in the downloaded `api_key.json` file.

During the OAuth flow, the `yup-oauth2` library automatically runs a temporary local HTTP server on
port 3030 to capture the authorization callback from Google.

#### 2. `token.json` - Access and Refresh Tokens

Generated after successful OAuth consent flow. The file contains:

```json
{
  "access_token": "ya29.a0AfH6SMBx...",
  "refresh_token": "1//0gHZnXz9dD8...",
  "token_type": "Bearer",
  "expiry": "2025-11-11T12:00:00Z"
}
```

### Authentication Commands

#### Initial Setup: `tiller auth`

The `tiller auth` command guides users through the OAuth consent flow:

1. **Load OAuth credentials** from `api_key.json`
2. **Validate redirect URI** - Ensures the file contains `http://localhost:3030` or `127.0.0.1:3030`
3. **Create authenticator** using `yup_oauth2::InstalledFlowAuthenticator` with:
    - Required scope: `https://www.googleapis.com/auth/spreadsheets` (read/write access)
    - `InstalledFlowReturnMethod::HTTPRedirect` on port 3030
4. **Trigger OAuth flow** - The `yup-oauth2` library automatically:
    - Generates the consent URL
    - Opens the user's browser to the authorization page
    - Starts a temporary HTTP server on `localhost:3030`
    - Captures the authorization callback from Google
    - Exchanges the authorization code for access and refresh tokens
    - Saves tokens to `token.json`
    - Shuts down the temporary server
5. **Confirm success** to user

**Error Handling:**

- If `api_key.json` is missing, provide clear instructions for obtaining it from Google Cloud
  Console
- If `api_key.json` doesn't contain the correct redirect URI (`http://localhost:3030`), display an
  error
- If OAuth flow times out or fails, display error and suggest retrying

#### Verification and Refresh: `tiller auth verify`

Verifies authentication and refreshes tokens if needed. (Note: We could also provide `tiller auth
refresh` as an alias or separate command, but `verify` captures the user intent - "check if my auth
is working" - and will refresh automatically if needed.)

Tests the current authentication state and refreshes tokens when necessary:

1. **Load credentials** from both `api_key.json` and `token.json`
2. **Create client** using loaded credentials
3. **Attempt API call** (e.g., get spreadsheet metadata using the configured `sheet_url` ID)
4. **Report results**:
    - Success: "Authentication verified successfully"
    - Token expired but refreshable: Automatically refresh and report success
    - Token invalid: "Authentication failed. Run 'tiller auth' to re-authenticate"

### Client Creation Pattern

All commands that interact with Google Sheets will use a consistent client creation pattern:

```rust
use google_sheets4::Sheets;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;

async fn create_sheets_client(
    api_key_path: &Path,
    token_path: &Path,
) -> Result<Sheets<HttpsConnector<HttpConnector>>> {
    // 1. Load api_key.json
    let api_key_content = fs::read_to_string(api_key_path)?;
    let api_key: ApiKeyFile = serde_json::from_str(&api_key_content)?;

    // 2. Create authenticator using yup-oauth2
    // The authenticator handles token loading, refresh, and persistence automatically
    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        api_key.installed.clone(),
        yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
        .persist_tokens_to_disk(token_path)
        .build()
        .await?;

    // 3. Create HTTPS connector
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()?
        .https_or_http()
        .enable_http1()
        .build();

    // 4. Create Google Sheets client
    let client = Sheets::new(
        hyper::Client::builder().build(connector),
        auth,
    );

    Ok(client)
}
```

### Automatic Token Refresh

The `yup-oauth2` library handles token refresh automatically. When creating the authenticator with
`persist_tokens_to_disk()`, it will:

1. **Load existing tokens** from `token.json` if available
2. **Check token expiration** before each API call
3. **Automatically refresh** if the access token is expired
4. **Save refreshed tokens** back to `token.json`
5. **Fail gracefully** if refresh fails (e.g., refresh token is invalid)

No manual token management is required - the library handles everything transparently.

### Required OAuth Scopes

The application requests the following scope during OAuth consent:

- `https://www.googleapis.com/auth/spreadsheets` - Read and write access to Google Sheets

This scope is sufficient for all operations (reading and writing to Transactions, Categories, and
AutoCat sheets).

### Security Considerations

1. **File Permissions**: Ensure `.secrets/` directory and credential files have restrictive
   permissions (0600 on Unix-like systems). The implementation sets these automatically on Unix
   systems.
2. **No Logging**: Never log credential values, tokens, or client secrets
3. **Error Messages**: Sanitize error messages to avoid leaking credential information
4. **Token Storage**: Store tokens as-is without additional encryption (filesystem permissions
   provide security). The `yup-oauth2` library manages token persistence.
5. **Redirect URI Security**: The `yup-oauth2` library's HTTP server for OAuth callback:
    - Binds only to `localhost` (127.0.0.1)
    - Shuts down immediately after receiving callback
    - Has built-in timeout handling

### First-Time Setup Flow

Expected user experience:

```bash
# Step 1: Set up OAuth credentials in Google Cloud Console
# (Users follow detailed instructions in SETUP.md)
# Download the client_secret_*.json file

# Step 2: Initialize the tiller directory
$ tiller init \
    --sheet-url "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID" \
    --api-key ~/Downloads/client_secret_*.json

Successfully created the tiller directory and config

# Step 3: Authenticate with Google
$ tiller auth
Opening browser for authorization...

If browser doesn't open automatically, visit:
https://accounts.google.com/o/oauth2/auth?client_id=...

Waiting for authorization...

✓ Authorization successful!
✓ Tokens saved to: /Users/you/tiller/.secrets/token.json

# Step 4: Verify authentication
$ tiller auth verify
✓ Authentication verified successfully
✓ Token is valid
  Spreadsheet: Tiller Foundation Template
  Access: Read/Write

# Now ready to sync!
$ tiller sync down
```

## Syncing Behavior

### Down

During the `tiller sync down` call, the following happens.

- If the datastore does not exist, it is created.
- A backup of the SQLite database is created.
- If more than `backup_copies` of the SQLite database exist, the extras are deleted.
- Three tabs from the `sheet_url`, *Transactions*, *Categories*, and *AutoCat*
- These are held in memory for further processing but also written out to
  `$TILLER_HOME/.backups/download.2025-11-09-001.json`.
- If there are more than `backup_copies` of `download.*.json` files, the oldest are deleted.
- Each of three tables in tiller.sqlite is upserted with the downloaded values.
    - Rows will be added to the database for new rows found in the sheets.
    - Rows will be deleted from the database for rows deleted from the sheets.
    - Rows will be updated in the database for rows that have been changed in the sheets.

### Up

- If the datastore does not exist, an error is raised suggesting that `down` should be used first.
- If the SQLite database is empty of transactions, an error is raised suggesting that `down` should
  be used first.
- A backup of the SQLite database is created.
- If more than `backup_copies` of the SQLite database exist, the extras are deleted.
- Each of the three tables in tiller.sqlite is used to upsert the Google sheet's corresponding tabs.
    - Rows will be added to the sheets for new rows found in the database.
    - Rows will be deleted from the sheets when their corresponding rows in the database have been
      deleted.
    - Rows will be updated when they match shows in the database.

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
local-f47e8c2a9b3d4f1ea8
```

This is a UUIDv4 with the dashes removed, 14-characters removed at random, and prepended with
`local-`.

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

### Transactions Table

The Transactions table stores all financial transactions synced from Tiller. Column descriptions are
from the [Tiller documentation](https://help.tiller.com/en/articles/432681).

| Column Name        | SQLite Type      | Description                                                           |
|--------------------|------------------|-----------------------------------------------------------------------| 
| `transaction_id`   | TEXT PRIMARY KEY | Unique Tiller-assigned identifier (or local- prefixed UUID)           |
| `date`             | TEXT             | Transaction posted or occurrence date (ISO 8601: YYYY-MM-DD)          |
| `description`      | TEXT             | Cleaned merchant/transaction details                                  |
| `amount`           | NUMERIC          | Transaction value; positive for income/credits, negative for expenses |
| `account`          | TEXT             | Account name from bank or user-assigned nickname                      |
| `account_number`   | TEXT             | Last four digits of account number (format: xxxx####)                 |
| `institution`      | TEXT             | Financial institution name                                            |
| `month`            | TEXT             | First day of transaction month for reporting (YYYY-MM-DD)             |
| `week`             | TEXT             | Sunday of transaction week for analysis (YYYY-MM-DD)                  |
| `full_description` | TEXT             | Unprocessed merchant data from bank                                   |
| `account_id`       | TEXT             | Unique account identifier for support                                 |
| `check_number`     | TEXT             | Check identifier when available                                       |
| `date_added`       | TEXT             | Spreadsheet entry date (YYYY-MM-DD)                                   |
| `merchant_name`    | TEXT             | Normalized merchant identifier across varied descriptions             |
| `category_hint`    | TEXT             | Data provider's category suggestion                                   |
| `category`         | TEXT             | Manual transaction categorization (user-added)                        |
| `note`             | TEXT             | User annotations for specific transactions                            |
| `tags`             | TEXT             | Additional transaction classification layer                           |

**Constraints:**

- Primary key: `transaction_id`
- All date fields stored as TEXT in ISO 8601 format (YYYY-MM-DD)
- `amount` uses NUMERIC type for decimal precision
- Only `transaction_id`, `date`, `description`, `amount`, `account`, `account_number`,
  `institution`,
  and `account_id` are considered required (NOT NULL)
- All other fields are optional (nullable)

**Indexes:**

```sql
CREATE INDEX idx_transactions_date ON transactions (date);
CREATE INDEX idx_transactions_account ON transactions (account);
CREATE INDEX idx_transactions_category ON transactions (category);
CREATE INDEX idx_transactions_description ON transactions (description);
```

**Notes:**

- Additional columns added to Tiller after initial sync will only populate for new transactions, not
  retroactively
- The sign convention for `amount` is: positive = income/credits, negative = expenses
- `Month` and `Week` fields are automatically calculated by Tiller for reporting/grouping purposes.
  If we add rows, we should calculate and populate them.

## Notes and Todos

This describes editing the transactions sheet:
https://help.tiller.com/en/articles/432679-editing-the-transactions-sheet

This is very important documentation describing each column and what it is for:
https://help.tiller.com/en/articles/432681-transactions-sheet-columns
