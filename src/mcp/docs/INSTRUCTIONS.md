# Tiller MCP Server Instructions

This MCP server synchronizes financial data between a Tiller Google Sheet and a local SQLite
database. The local database serves as a workspace for analysis and manipulation before syncing
changes back to the sheet.

## Prerequisites

The user must have already completed CLI setup before using these tools:

1. `tiller init` - Creates the local directory structure and configuration
2. `tiller auth` - Completes OAuth authentication with Google

If these steps haven't been completed, the tools will fail with authentication errors.

The user can follow the README at https://github.com/webern/tiller-sync for help with setup.

## Data Types

Three types of data are synchronized:

| Data Type        | Description                                     | Sync Semantics       |
|------------------|-------------------------------------------------|----------------------|
| **Transactions** | Financial transactions (date, amount, category) | Upsert (incremental) |
| **Categories**   | Budget categories and settings                  | Full replacement     |
| **AutoCat**      | Automatic categorization rules                  | Full replacement     |

**Upsert** means new rows are inserted, existing rows are updated, and deleted rows are removed.
**Full replacement** means all existing rows are deleted and replaced with the incoming data.

## Recommended Workflow

```
1. sync_down          <- Download latest data from Google Sheet
2. [analyze/edit]     <- Work with local SQLite database
3. sync_up            <- Upload changes back to Google Sheet
```

Always run `sync_down` before making local edits to ensure you have the latest data and to
establish a baseline for conflict detection.

## Tool Reference

### `sync_down`

Downloads data from the Google Sheet to the local SQLite database.

**Parameters:** None

**Backups created:**

- `tiller.sqlite.YYYY-MM-DD-NNN` - Copy of the existing database
- `sync-down.YYYY-MM-DD-NNN.json` - Snapshot of downloaded data (used for conflict detection)

**Behavior:**

- Transactions are upserted (insert/update/delete based on Transaction ID)
- Categories and AutoCat are fully replaced
- Cell formulas are captured and stored for optional preservation during `sync_up`
- Each row's `original_order` is recorded for formula position tracking

**Caution:** This overwrites local changes. The SQLite backup enables manual recovery if needed.

### `sync_up`

Uploads data from the local SQLite database to the Google Sheet.

**Parameters:**

| Parameter  | Type    | Default   | Description                                      |
|------------|---------|-----------|--------------------------------------------------|
| `force`    | boolean | `false`   | Override conflict detection and formula warnings |
| `formulas` | string  | `unknown` | Formula handling mode (see below)                |

**Backups created (before any writes):**

1. `sync-up-pre.YYYY-MM-DD-NNN.json` - Current sheet state before modification
2. `tiller.sqlite.YYYY-MM-DD-NNN` - Copy of the local database
3. Google Sheet copy via Drive API (`tiller-backup-YYYY-MM-DD-HHMMSS`)

**Strategy:** The local database is treated as the authoritative source. The tool clears all sheet
data and writes the complete dataset from SQLite.

## Conflict Detection

Before uploading, `sync_up` compares the current Google Sheet against the last `sync_down` backup:

| Scenario                        | Without `force`                           | With `force=true`        |
|---------------------------------|-------------------------------------------|--------------------------|
| Sheet unchanged since sync_down | Proceeds normally                         | Proceeds normally        |
| Sheet modified since sync_down  | **Error**: "Sheet has been modified..."   | Proceeds (overwrites)    |
| No sync_down backup exists      | **Error**: "No sync-down backup found..." | Skips conflict detection |

**Recommendation:** Only use `force=true` when you are certain the local database should completely
replace the remote sheet, discarding any remote changes.

## Formula Handling

Tiller sheets may contain formulas (e.g., balance calculations, conditional formatting). The
`formulas` parameter controls how these are handled during `sync_up`:

| Mode       | Behavior                                                                      |
|------------|-------------------------------------------------------------------------------|
| `unknown`  | **Error** if formulas exist, prompting explicit choice of `preserve`/`ignore` |
| `preserve` | Write formulas back to their original cell positions                          |
| `ignore`   | Skip all formulas; only write values                                          |

### Formula Preservation Details

When `formulas=preserve`:

- Formulas are written to their original (row, column) positions from the last `sync_down`
- Row positions are tracked via the `original_order` field

**Gap Detection:** If rows have been deleted locally, there will be gaps in `original_order`
(e.g., 0, 1, 3 instead of 0, 1, 2). This means formula positions may be incorrect because the
sheet rows have shifted.

| Gaps Detected | Without `force`                        | With `force=true`             |
|---------------|----------------------------------------|-------------------------------|
| No gaps       | Proceeds normally                      | Proceeds normally             |
| Gaps exist    | **Error**: "Row deletions detected..." | Proceeds (formulas may break) |

**Recommendation:** If you've deleted rows and have formulas, use `formulas=ignore` unless you
understand the implications of misaligned formula positions.

## Error Handling

Common errors and resolutions:

| Error                              | Cause                               | Resolution                            |
|------------------------------------|-------------------------------------|---------------------------------------|
| "Database has no transactions"     | Empty local database                | Run `sync_down` first                 |
| "No sync-down backup found"        | Never ran `sync_down`               | Run `sync_down` or use `force=true`   |
| "Sheet has been modified since..." | Remote changes detected             | Run `sync_down` or use `force=true`   |
| "Formulas detected in database"    | Formulas exist, mode is `unknown`   | Set `formulas` to `preserve`/`ignore` |
| "Row deletions detected"           | Gaps in order + `formulas=preserve` | Use `force=true` or `formulas=ignore` |

## Verification

After `sync_up` writes data, it re-fetches row counts from each sheet tab and verifies they match
what was written. The tool reports the final counts on success.

## Query Interface

The `query` and `schema` tools provide read-only access to the local SQLite database. This enables
AI agents to analyze financial data, generate reports, and understand the data structure without
modifying anything.

### `query`

Executes a read-only SQL query against the local SQLite database.

**Parameters:**

| Parameter | Type   | Required | Description                                           |
|-----------|--------|----------|-------------------------------------------------------|
| `sql`     | string | Yes      | The SQL query to execute (SELECT only)                |
| `format`  | string | Yes      | Output format: `json`, `markdown`, or `csv`           |

**Output Formats:**

| Format     | Description                                                          |
|------------|----------------------------------------------------------------------|
| `json`     | JSON array of objects, each row is an object with column names as keys |
| `markdown` | Markdown table suitable for display                                  |
| `csv`      | CSV format with header row followed by data rows                     |

**Security:** The query interface uses a read-only SQLite connection (`?mode=ro`). Any write
attempt (INSERT, UPDATE, DELETE) will be rejected by SQLite.

**Warning:** Large result sets are returned in full. Use `LIMIT` clauses for potentially large
queries.

**Example Queries:**

```sql
-- Recent transactions
SELECT date, description, amount, category
FROM transactions
ORDER BY date DESC
LIMIT 20

-- Spending by category
SELECT category, SUM(amount) as total
FROM transactions
WHERE amount < 0
GROUP BY category
ORDER BY total

-- Uncategorized transactions
SELECT date, description, amount
FROM transactions
WHERE category IS NULL OR category = ''
ORDER BY date DESC

-- Transaction counts by month
SELECT strftime('%Y-%m', date) as month, COUNT(*) as count
FROM transactions
GROUP BY month
ORDER BY month DESC
```

### `schema`

Retrieves database schema information to help understand the data structure.

**Parameters:**

| Parameter          | Type    | Default | Description                              |
|--------------------|---------|---------|------------------------------------------|
| `include_metadata` | boolean | `false` | Include internal metadata tables         |

**Data Tables:**

| Table          | Description                         | Primary Key       |
|----------------|-------------------------------------|-------------------|
| `transactions` | Financial transactions              | `transaction_id`  |
| `categories`   | Budget categories                   | `category`        |
| `autocat`      | Automatic categorization rules      | `id` (auto-incr)  |

**Metadata Tables** (when `include_metadata=true`):

| Table            | Description                                  |
|------------------|----------------------------------------------|
| `sheet_metadata` | Column ordering and header mapping per sheet |
| `formulas`       | Cell formulas from the Google Sheet          |
| `schema_version` | Database schema version tracking             |

**Schema Output:** Returns JSON with detailed information about each table:

- `tables`: Array of table info objects containing:
  - `name`: Table name
  - `row_count`: Number of rows
  - `columns`: Column details (name, type, nullable, primary key, description)
  - `indexes`: Index information
  - `foreign_keys`: Foreign key relationships

**Key Columns in `transactions`:**

| Column           | Type    | Description                                          |
|------------------|---------|------------------------------------------------------|
| `transaction_id` | TEXT    | Unique ID (Tiller-assigned or `user-` prefixed)      |
| `date`           | TEXT    | Transaction date (YYYY-MM-DD)                        |
| `description`    | TEXT    | Cleaned merchant description                         |
| `amount`         | TEXT    | Transaction amount (negative = expense)              |
| `category`       | TEXT    | Assigned category (FK to categories)                 |
| `account`        | TEXT    | Account name                                         |
| `institution`    | TEXT    | Financial institution                                |
| `note`           | TEXT    | User notes                                           |
| `original_order` | INTEGER | Row position from last sync (for formula tracking)   |

## Best Practices

1. **Always sync down first** - Establishes baseline for conflict detection and ensures fresh data
2. **Use `formulas=ignore` when uncertain** - Safest option if you don't need formula preservation
3. **Avoid `force=true` casually** - It bypasses safety checks; use only when intentional
4. **Use `schema` before complex queries** - Understand the data structure before writing SQL
5. **Use `LIMIT` in exploratory queries** - Prevent overwhelming responses with large result sets
