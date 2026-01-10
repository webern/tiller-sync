# Tiller Sync

A CLI tool and MCP server for syncing data between a [Tiller][tiller] Google Sheet and a local
SQLite database. Download your transactions, query and edit them locally, then sync changes back to
your sheet - all from the command line or through an AI agent.

## Features

- **Bidirectional sync**: Download transactions from your Tiller sheet and upload local changes back
- **Local SQLite database**: Query and manipulate your financial data with SQL
- **CRUD operations**: Create, read, update, and delete transactions, categories, and AutoCat rules
- **Query interface**: Execute arbitrary SQL queries with JSON, Markdown, or CSV output
- **MCP server**: Integrate with AI agents like Claude Code for automated financial analysis

## Table of Contents

- [Installation](#installation)
    - [For Non-Rust Users](#for-non-rust-users)
- [Setup](#setup)
    - [Prerequisites](#prerequisites)
    - [Initial Setup](#initial-setup)
- [Usage](#usage)
- [Claude Code Integration](#claude-code-integration)
- [Troubleshooting](#troubleshooting)

## Installation

Install Tiller Sync using Cargo:

```bash
cargo install tiller-sync
```

The binary will be installed to `$CARGO_HOME/bin/tiller` (typically `~/.cargo/bin/tiller`).

### For Non-Rust Users

If you don't have Rust installed, you'll need to install it first:

1. Visit [rustup.rs](https://rustup.rs/) and follow the installation instructions
2. After installation, ensure `$CARGO_HOME/bin` is in your PATH (the installer usually does this
   automatically)
3. Open a new terminal and run `cargo install tiller-sync`

**Note**: `CARGO_HOME` defaults to `~/.cargo` on Unix systems and `%USERPROFILE%\.cargo` on Windows.
The installed binary will be at `$CARGO_HOME/bin/tiller`.

## Setup

### Prerequisites

- A [Tiller](https://tiller.com/) account with an active Google Sheets subscription
- A Google account (the same one used for Tiller)
- Access to [Google Cloud Console](https://console.cloud.google.com/)

### Initial Setup

Setting up Tiller Sync requires a few steps. Please follow them in order:

#### 1. Set up Google Cloud Console

First, you need to create OAuth credentials in Google Cloud Console. This process is somewhat
involved but only needs to be done once.

**Follow the detailed instructions in [SETUP.md](docs/SETUP.md)** to:

- Create a Google Cloud project
- Enable the Google Sheets API
- Configure OAuth consent screen
- Create and download OAuth credentials

Once you've completed those steps and have your downloaded `client_secret_*.json` file, return here
to continue.

#### 2. Initialize Tiller

After completing the Google Cloud Console setup, initialize your Tiller directory with the
`tiller init` command. You'll need:

- The path to your downloaded OAuth credentials file (from step 1)
- The URL of your Tiller Google Sheet

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

This command will:

- Create the data directory structure
- Copy your OAuth credentials to `.secrets/client_secret.json`
- Create an initial `config.json` with your sheet URL

#### 3. Authenticate with Google

Now authenticate Tiller Sync to access your Google Sheets:

```bash
tiller auth
```

The command will:

- Automatically open your web browser to Google's authorization page
- If the browser doesn't open automatically, copy the URL displayed in the terminal

In the browser:

- Select the Google account you use for Tiller
- You may see a warning that "Google hasn't verified this app"
    - Click **"Advanced"**
    - Click **"Go to Tiller Sync (unsafe)"**
    - This warning appears because you created the OAuth credentials yourself - your data is safe
- Review the permissions requested
- Click **"Allow"**

After clicking "Allow", you should see a success message in your browser and in your terminal:

```
✓ Authorization successful!
✓ Tokens saved to: /Users/you/tiller/.secrets/token.json
```

Verify your authentication:

```bash
tiller auth verify
```

You should see:

```
✓ Authentication verified successfully
  Spreadsheet: Tiller Foundation Template
  Access: Read/Write
```

**You're all set!** You can now use the sync commands below.

## Usage

### Sync Data from Google Sheets to Local Database

Download your Tiller data to a local SQLite database:

```bash
tiller sync down
```

This will:

- Create a local SQLite database at `~/tiller/tiller.sqlite` (if it doesn't exist)
- Download Transactions, Categories, and AutoCat data from your Tiller sheet
- Create a backup of the previous database state

### Sync Local Changes to Google Sheets

Upload local changes back to your Tiller sheet:

```bash
tiller sync up
```

This will:

- Update your Google Sheets with any changes made to the local database
- Create a backup before syncing

### Query Data

Execute SQL queries against your local database:

```bash
# Query recent transactions (JSON output)
tiller query "SELECT date, description, amount FROM transactions ORDER BY date DESC LIMIT 10"

# Get results as a markdown table
tiller query --format markdown "SELECT category, SUM(amount) as total FROM transactions GROUP BY category"

# Export to CSV
tiller query --format csv "SELECT * FROM transactions WHERE amount < 0" > expenses.csv
```

### View Database Schema

```bash
# Show data tables (transactions, categories, autocat)
tiller schema

# Include metadata tables
tiller schema --include-metadata
```

### Configuration

The default configuration file is located at `~/tiller/config.json`. You can customize:

- **tiller_sheet**: URL of your Tiller Google Sheet
- **backup_copies**: Number of backup copies to retain (default: 5)
- **client_secret_path**: Custom path to `client_secret.json` (relative or absolute)
- **token_path**: Custom path to `token.json` (relative or absolute)

Example configuration:

```json
{
  "app_name": "tiller",
  "config_version": "v0.1.0",
  "tiller_sheet": "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID",
  "backup_copies": 5,
  "client_secret_path": ".secrets/client_secret.json",
  "token_path": ".secrets/token.json"
}
```

### Custom Tiller Home Directory

By default, Tiller Sync uses `~/tiller` as the home directory. You can override this:

```bash
# Using environment variable
export TILLER_HOME=/path/to/custom/location
tiller sync down

# Using command-line flag
tiller --dir /path/to/custom/location sync down
```

## Claude Code Integration

Tiller Sync includes an MCP (Model Context Protocol) server that allows AI agents like Claude Code
to interact with your financial data.

### Adding the MCP Server

After installing Tiller Sync and completing the setup steps above, add it as an MCP server to Claude
Code:

```bash
claude mcp add tiller -- tiller --tiller-home ~/tiller mcp
```

If you're using a custom tiller home directory, adjust the path accordingly:

```bash
claude mcp add tiller -- tiller --tiller-home /path/to/your/tiller mcp
```

### Available MCP Tools

Once configured, Claude Code can use the following tools:

- **sync_down** / **sync_up**: Sync data between your Google Sheet and local database
- **query**: Execute SQL queries against your local database
- **schema**: View database structure and column descriptions
- **insert_transaction** / **update_transactions** / **delete_transactions**: Manage transactions
- **insert_category** / **update_categories** / **delete_categories**: Manage categories
- **insert_autocat** / **update_autocats** / **delete_autocats**: Manage AutoCat rules

### Example Use Cases

With Claude Code and Tiller Sync, you can:

- Ask Claude to analyze your spending patterns
- Have Claude suggest AutoCat rules for uncategorized transactions
- Request spending reports by category, merchant, or time period
- Get help identifying areas to reduce expenses

## Troubleshooting

### "Access token expired" or "Invalid credentials"

Your OAuth token may have expired. Refresh it with:

```bash
tiller auth verify
```

If that doesn't work, re-authenticate:

```bash
tiller auth
```

### "Client secret file not found"

Ensure you've placed `client_secret.json` in the correct location:

```bash
ls -la ~/tiller/.secrets/client_secret.json
```

If the file is missing, you'll need to download the OAuth credentials again from Google Cloud
Console.

### "Google hasn't verified this app" warning

This is expected for personal OAuth applications. To proceed:

1. Click **"Advanced"**
2. Click **"Go to Tiller Sync (unsafe)"**

This warning appears because you created the OAuth credentials yourself rather than using a
verified public application. Your data is safe - you're authenticating with your own credentials.

### Browser doesn't open during authentication

If the browser doesn't open automatically during `tiller auth`:

1. Look for the authorization URL in the terminal output
2. Copy the URL manually
3. Paste it into your browser
4. Complete the authorization flow

### Permission denied errors

Ensure credential files have the correct permissions:

```bash
chmod 600 ~/tiller/.secrets/client_secret.json
chmod 600 ~/tiller/.secrets/token.json
```

### Need more help?

- Check the [GitHub Issues](https://github.com/webern/tiller-sync/issues)
- Review the [design documentation](docs/DESIGN.md)
- Open a new issue with details about your problem

<!-- @formatter:off -->

[tiller]: https://tiller.com/
