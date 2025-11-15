# Tiller Sync

A program for syncing data between a [tiller] Google Sheet and a local SQLite database.

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Setup](#setup)
- [API Setup](docs/SETUP.md)
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

Create a directory for configuration and data. By default, this can be created at `$HOME/tiller`
with:

```bash
tiller init
```

Or you can put it wherever you want with:

```bash
tiller init --tiller-home /wherever/you/want/mytillerdir
```

Unfortunately, setting up Google API access is rather extensive. I have tried to make the
instructions as precise as possible. Please see the [setup](docs/SETUP.md) doc and follow the
instructions there
carefully.

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
- **api_key_path**: Custom path to `api_key.json` (relative or absolute)
- **token_path**: Custom path to `token.json` (relative or absolute)

Example configuration:

```json
{
  "app_name": "tiller",
  "config_version": "v0.1.0",
  "tiller_sheet": "https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID",
  "backup_copies": 5,
  "api_key_path": ".secrets/api_key.json",
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

### "API key file not found"

Ensure you've placed `api_key.json` in the correct location:

```bash
ls -la ~/tiller/.secrets/api_key.json
```

If the file is missing, repeat [Step 5: Download Credentials](#step-5-download-credentials).

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
chmod 600 ~/tiller/.secrets/api_key.json
chmod 600 ~/tiller/.secrets/token.json
```

### Need more help?

- Check the [GitHub Issues](https://github.com/webern/tiller-sync/issues)
- Review the [design documentation](docs/DESIGN.md)
- Open a new issue with details about your problem

<!-- @formatter:off -->

[tiller]: https://tiller.com/
