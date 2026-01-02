# Tiller Sync

⚠️ Not ready yet!

A program for syncing data between a [tiller] Google Sheet and a local SQLite database.
With it you can download your transactions, make edits to them locally in SQLite, then sync back the
changes (this part is hard, TBD!)

Current Status: `tiller sync down` and `tiller sync up` are working and the MCP server is working.
This means you can store your transactions locally and can upload them back up to your sheet, either
with the command line or with an AI agent via MCP, but you cannot manipulate them (unless you use
SQLite).

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Setup](#setup)
    - [Prerequisites](#prerequisites)
    - [Initial Setup](#initial-setup)
- [Usage](#usage)
- [Troubleshooting](#troubleshooting)

## Overview

Tiller Sync is a Rust CLI tool that allows you to sync financial data between your Tiller Google
Sheet and a local SQLite database. This enables offline access, custom queries, and data
manipulation
of your Tiller financial data.

## Installation

TODO: this will be published to crates.io and possible also as a binary release.

```bash
# Clone the repository
git clone https://github.com/webern/tiller-sync.git
cd tiller-sync

# Build and install
cargo install --path .
```

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

### Query Transactions

*(Coming soon - MCP and query commands are in development)*

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
