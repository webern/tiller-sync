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
    - Contains helper functions for resolving credential file paths (client_secret.json, token.json)
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
  --client-secret ~/Downloads/client_secret_*.json

# Or specify a custom location
tiller init \
  --tiller-home /path/to/custom/location \
  --sheet-url "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID" \
  --client-secret ~/Downloads/client_secret_*.json
```

The `tiller init` command:

- Creates the data directory structure (including `.secrets/` and `.backups/` subdirectories)
- **Moves** the OAuth credentials file to `.secrets/client_secret.json`
- Creates an initial `config.json` with the provided sheet URL and default settings

**Arguments:**

- `--sheet-url`: URL of the user's Tiller Google Sheet (required)
- `--client-secret`: Path to the downloaded OAuth 2.0 client credentials file from Google Cloud Console (required)
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
│   ├── client_secret.json
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
  "client_secret_path": ".secrets/client_secret.json",
  "token_path": ".secrets/token.json"
}
```

**Note:** The `config_version` value is not intended to match the release version. It is an
independent identifier that can be used to handle breaking deserialization changes in the future.

The `client_secret_path` and `token_path` fields are optional. Paths can be absolute or relative to
the `config.json` file. If omitted, they default to `$TILLER_HOME/.secrets/client_secret.json` and
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

### Library Selection

The implementation uses:

- **`oauth2`** - OAuth 2.0 authentication flow implementation (provides full control over the auth process)
- **OxideComputer's `sheets` library** - Google Sheets API client library

**Explicitly NOT using:**

- **`yup-oauth2`** - This library does not provide sufficient control over when and how the OAuth
  browser interaction occurs. We need explicit control to ensure only `tiller auth` can initiate
  interactive authentication.
- **`google-sheets4`** - This crate is tightly coupled to `yup-oauth2` and inherits the same
  limitations around authentication control.

**Why manual OAuth implementation:**

The `oauth2` crate approach provides explicit control over when user interaction is required versus
when non-interactive token refresh should be attempted. The application decides whether to enter an
interactive OAuth flow or fail with a clear error message. This is essential for our architecture
where:
- `tiller auth` is the sole command that can open a browser for user authentication
- All other commands must remain non-interactive and scriptable
- MCP mode must never prompt for user interaction

### Authentication Control Philosophy

**Only `tiller auth` initiates user authentication workflows.** This command is the sole entry point
for interactive OAuth consent flows that require opening a browser and user interaction.

**All other commands** (`tiller sync up`, `tiller sync down`, `tiller auth --verify`, etc.) will:
- Automatically refresh tokens when they expire (non-interactive)
- Raise clear errors if authentication fails, instructing users to run `tiller auth`
- Never prompt for user interaction or open browsers

This design ensures that:
1. MCP mode and sync operations remain non-interactive and scriptable
2. Users have clear, predictable control over when authentication occurs
3. Error messages provide actionable guidance ("Run 'tiller auth' to re-authenticate")

### Code Organization

All Google Sheets and Google Auth related code will be located in `src/api/`. This includes:

- OAuth authentication flow implementation (`src/api/oauth.rs`)
- Google Sheets client wrapper (`src/api/sheets_client.rs`)
- Credential file structures (`src/api/files.rs`)
- Token management (manual implementation using the `oauth2` crate)

To enable testing, we will wrap sheets and OAuth operations in a trait that can be mocked and
injected. The production code will use implementations backed by `oauth2` and OxideComputer's sheets
library, while tests will use mock implementations.

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

**Important**: When creating OAuth credentials in Google Cloud Console, users must set the redirect
URI to `http://localhost`. This URI must match exactly what is configured in the Google Cloud
Console and what appears in the downloaded `client_secret.json` file.

During the OAuth flow, the application automatically runs a temporary local HTTP server on a random
available port to capture the authorization callback from Google.

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

The `tiller auth` command guides users through the OAuth consent flow. This is the **only** command
that will initiate an interactive user authentication workflow.

The command performs the following steps:

1. **Delete existing token** - If `token.json` exists, delete it to ensure a fresh authentication
2. **Load OAuth credentials** from `client_secret.json`
3. **Validate redirect URI** - Ensures the file contains the configured redirect URI
4. **Create OAuth client** using the `oauth2` crate
5. **Generate authorization URL** with required scope: `https://www.googleapis.com/auth/spreadsheets`
6. **Start local HTTP server** to receive the OAuth callback
7. **Open user's browser** to the authorization URL
8. **Wait for callback** - The local server captures the authorization code
9. **Exchange code for tokens** - Request access and refresh tokens from Google
10. **Save tokens** to `token.json`
11. **Shut down local server**
12. **Confirm success** to user

**Error Handling:**

- If `client_secret.json` is missing, provide clear instructions for obtaining it from Google Cloud
  Console
- If `client_secret.json` doesn't contain the correct redirect URI, display an error
- If OAuth flow times out or fails, display error and suggest retrying

**Important:** This command always deletes any existing `token.json` before starting, ensuring that
each run performs a complete fresh authentication.

#### Verification and Refresh: `tiller auth --verify`

Verifies authentication and refreshes tokens if needed. This command does **not** initiate
interactive user authentication - it only attempts non-interactive token refresh.

Tests the current authentication state and refreshes tokens when necessary:

1. **Load credentials** from both `client_secret.json` and `token.json`
2. **Check if token is expired** - If not expired, proceed to step 3
3. **Attempt token refresh** (non-interactive) - If token is expired, use refresh token to get new access token
4. **Create sheets client** using the credentials
5. **Attempt API call** - Get spreadsheet metadata using the configured `sheet_url` ID
6. **Report results**:
    - Success: "Authentication verified successfully"
    - Token refreshed: "Token refreshed successfully"
    - Authentication failed: "Authentication failed. Run 'tiller auth' to re-authenticate"

**Important:** If token refresh fails or tokens are invalid, this command will **not** open a
browser or prompt for user interaction. It will display a clear error message instructing the user
to run `tiller auth`.

### Client Creation Pattern

All commands that interact with Google Sheets will use a consistent client creation pattern:

```rust
use oxide_auth::primitives::prelude::*;
// OxideComputer sheets library imports (specifics TBD based on actual API)

async fn create_sheets_client(
    secret_path: &Path,
    token_path: &Path,
) -> Result<SheetsClient> {
    // 1. Load client_secret.json
    let secret_content = fs::read_to_string(secret_path)?;
    let secret: ClientSecret = serde_json::from_str(&secret_content)?;

    // 2. Load token.json
    let token_content = fs::read_to_string(token_path)
        .map_err(|_| Error::NotAuthenticated("Run 'tiller auth' to authenticate"))?;
    let mut token: TokenData = serde_json::from_str(&token_content)?;

    // 3. Check if token is expired and refresh if needed
    if token.is_expired() {
        token = refresh_token(&secret, &token)
            .await
            .map_err(|_| Error::RefreshFailed("Run 'tiller auth' to re-authenticate"))?;

        // Save refreshed token
        save_token(token_path, &token)?;
    }

    // 4. Create Google Sheets client with the access token
    let client = SheetsClient::new(token.access_token)?;

    Ok(client)
}
```

### Token Refresh Behavior

Token refresh is handled manually using the `oauth2` crate, providing explicit control over the
refresh process:

**Non-Interactive Commands** (`tiller sync up`, `tiller sync down`, `tiller auth --verify`):
1. **Load existing tokens** from `token.json`
2. **Check token expiration** using the stored expiry timestamp
3. **Attempt non-interactive refresh** if the access token is expired using the refresh token
4. **Save refreshed tokens** back to `token.json` on success
5. **Fail with clear error** if refresh fails, instructing user to run `tiller auth`

**Interactive Command** (`tiller auth` only):
1. **Delete existing token.json** to start fresh
2. **Perform full OAuth flow** with user interaction (browser-based consent)
3. **Save new tokens** to `token.json`

This approach ensures predictable behavior:
- Sync operations never open browsers or require user interaction
- Token refresh happens automatically but non-interactively
- Clear error messages guide users when re-authentication is needed
- Only `tiller auth` can initiate user-facing authentication workflows

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
   provide security). The application manages token persistence manually.
5. **Redirect URI Security**: The OAuth callback HTTP server (used only by `tiller auth`):
    - Binds only to `localhost` (127.0.0.1)
    - Shuts down immediately after receiving callback
    - Has timeout handling to prevent hanging
6. **Token Refresh Security**: Non-interactive token refresh uses HTTPS POST requests directly to
   Google's token endpoint with the refresh token. No user credentials are transmitted during
   refresh operations.

### First-Time Setup Flow

Expected user experience:

```bash
# Step 1: Set up OAuth credentials in Google Cloud Console
# (Users follow detailed instructions in SETUP.md)
# Download the client_secret_*.json file

# Step 2: Initialize the tiller directory
$ tiller init \
    --sheet-url "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID" \
    --client-secret ~/Downloads/client_secret_*.json

Successfully created the tiller directory and config

# Step 3: Authenticate with Google
$ tiller auth
Opening browser for authorization...

If browser doesn't open automatically, visit:
https://accounts.google.com/o/oauth2/auth?client_id=...

Waiting for authorization...

✓ Authorization successful!
✓ Tokens saved to: /Users/you/tiller/.secrets/token.json

# Step 4: Verify authentication (optional)
$ tiller auth --verify
✓ Authentication verified successfully
✓ Token is valid
  Spreadsheet: Tiller Foundation Template
  Access: Read/Write

# Now ready to sync!
$ tiller sync down

# If authentication ever fails during sync operations:
$ tiller sync down
Error: Authentication failed. Access token expired and refresh failed.
Run 'tiller auth' to re-authenticate.

$ tiller auth
[Browser opens for re-authentication...]
✓ Authorization successful!
✓ Tokens saved to: /Users/you/tiller/.secrets/token.json

$ tiller sync down
[Sync proceeds normally, automatically refreshing token if needed]
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
