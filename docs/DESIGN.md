# Tiller Design

The tiller app provides two main modes of operation, one for syncing data between a local datastore
and the user's Tiller Google sheet, and one that provides the MCP interface so that it can be used
as an MCP tool.

### Design Principles

- **Separation of Concerns**: API code focuses on API operations; configuration code handles paths
  and settings
- **Testability**: API operations use traits to enable mocking without requiring actual Google
  sheets interactions

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

1. **Delete existing token** - If `token.json` exists, delete it to ensure a fresh authentication
2. **Load OAuth credentials** from `client_secret.json`
3. **Validate redirect URI** - Ensures the file contains the configured redirect URI
4. **Create OAuth client** using the `oauth2` crate
5. **Generate authorization URL** with required scope(s):
6. **Start local HTTP server** to receive the OAuth callback
7. **Open user's browser** to the authorization URL
8. **Wait for callback** - The local server captures the authorization code
9. **Exchange code for tokens** - Request access and refresh tokens from Google
10. **Save tokens** to `token.json`
11. **Shut down local server**
12. **Confirm success** to user

**Important**: `tiller auth` is the only CLI command that initiates this interactive workflow. Every
other command is expected to be scriptable, and should simply error out if OAuth authentication does
not work.

#### `tiller auth --verify`

To check authentication, and refresh the token, users can call `tiller auth --verify`. The
`--verify` flag ensures that the command is non-interactive and will either

- Error if authentication does not work, or
- Report success if authentication worked and the token was refreshed.

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

The only command that works without a pre-existing datastore directory is `tiller init`. Every other
command will error out if the directory or config file cannot be found.

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

The problem with `yup-oauth2` is that it would automatically enter the interactive workflow if there
was any problem with authentication or missing scopes, etc. Claude and I could *not* find a
reasonable way to prevent this and I do not want this happening during CLI commands that are
expected to be non-interactive. Furthermore, the `google-sheets4` crate was deeply coupled to
`yup-oauth2`.

Thus: we decided to use `oauth2` and `sheets`.

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
URI to `http://localhost`. This is automatically provided if the user selects "Desktop App" when
creating the credentials. We enforce this requirement during deserialization.

During the OAuth flow, the application automatically runs a temporary local HTTP server on a random
available port to capture the authorization callback from Google.

#### 2. `token.json` - Access and Refresh Tokens

Generated after successful OAuth consent flow. The file contains:

```json
{
  "scopes": [
    "https://www.googleapis.com/auth/spreadsheets",
    "https://www.googleapis.com/auth/drive.readonly"
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
