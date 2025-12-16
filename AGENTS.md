# Tiller Project AI Agent Instructions

This file provides guidance to AI agents when working with code in this repository.

- You are the AI Agent
- I am the user

`.claude/CLAUDE.md` is a symlink to `./AGENTS.md`.

The root of this repo is:

- `.` relative to `AGENTS.md`
- `..` relative to `.claude/CLAUDE.md`

Paths in this file are relative to the root of the repo.

## Project Overview

The project sourcecode is hosted on GitHub at https://github.com/webern/tiller-sync.

Tiller Sync is a Rust CLI tool for syncing data between a [Tiller](https://tiller.com/) Google Sheet
and a local SQLite database.

## Design and Architecture

See @./docs/DESIGN.md for design and architecture.

## Status and Progress: Current State

This section is here to help AI agents understand what we have been working on and what we need to
do next.

What's Already Built for `tiller sync up`

1. Data Download (sync down) - Complete
2. Model Layer - Complete
3. API Layer - Partial
    - Sheet trait has get(), get_formulas() (implemented) and `_put()` (stub with todo!())
    - GoogleSheet implements these for real Google Sheets API calls
    - TestSheet provides in-memory test data
    - Tiller trait has get_data() only - no put_data() method yet
4. OAuth & Config - Complete
5. Database migrations and initial schema creation

Glossary:

- GENERAL: general steps related to design, quality or other non-specific taks
- SYNC_DOWN: work related to `tiller sync down` logic
- SYNC_UP: work related to `tiller sync up` logic
- BACKUP: work related to backup logic
- SQL: work related to the SQLite datastore

Next Steps:

- [X] GENERAL: Investigate the existing code base to understand where we left off.
- [X] SYNC_DOWN: Precondition checks - Verify datastore exists with transactions
- [X] BACKUP: Backup SQLite database
- [X] BACKUP: Add `sync-down.*.json` backup logic to `tiller sync down`
- [X] SYNC_UP: Download current sheet state - Save to sync-up-pre.*.json backup
- [X] SQL: Choose a Rust library for SQLite operations and add it to the project and to `Db`
- [X] SQL: Create a schema migration design and add it to DESIGN.md
- [X] SQL: Create an implementation plan for building the SQLite datastore and add the steps to
  AGENTS.md. The plan should divide up the implementation into 5-10 discrete steps.
- [X] SQL: Create src/db/migrations/ directory, CURRENT_VERSION constant in db/mod.rs
- [X] SQL: Implement bootstrap logic (creates schema_version table, inserts version 0)
- [X] SQL: Write a test of the bootstrap logic
- [X] SQL: Implement the logic that detects the required migrations and executes them
- [X] SQL: Create Migration 1, research Tiller documentation and our model structs for table
  structure
- [X] SQL: Write tests for the migration system
- [X] SQL: Wire migration logic into Db::init() - create a shared function that can also be used by
  load()
- [X] SQL: Wire migration logic into Db::load()
- [X] SQL Add sheet metadata table to Migration 1
- [X] SQL Add `original_order` column to tables in Migration 1
- [X] SQL Add `other_fields` columns to Migration 1
- [X] SQL Add `formulas` table to Migration 1
- [X] SQL Change `categories` table to use synthetic primary key in Migration 1
- [X] SQL TDD: Stub `db.save_tiller_data` and `db.get_tiller_data` functions with `todo!()` function
  bodies and write TWO basic tests. One for each function. These tests will fail at first (think
  Red/Green TDD).
- [X] SQL TDD: Stub `db.insert_transaction`, `db.update_transaction`, and `db.get_transaction`
  functions with `todo!()` bodies and failing Red/Green tests.
- [X] SQL TDD: Stub `db.insert_category`, `db.update_category`, and `db.get_category` functions with
  `todo!()` bodies and failing Red/Green tests.
- [X] SQL TDD: Stub `db.insert_autocat`, `db.update_autocat`, and `db.get_autocat` functions with
  `todo!()` bodies and failing Red/Green tests.
- [X] SQL TDD: Think of and propose more tests that will check nuances of the logic of these
  functions and write them.
- [X] SQL: Implement stubbed functions and get the tests to pass.

- [ ] STOP HERE: We need to design the interface and logic for Upserting data and querying data
  with Db (DESIGN COMPLETE - see "Db Interface Design Decisions" section below)

NO: WE ARE NOT READY FOR THESE YET

- [ ] SYNC_UP: Implement gap detection logic for original_order
- [ ] SYNC_UP: Build output data - Convert model objects to `Vec<Vec<String>>`
- [ ] SYNC_UP: Conflict detection - Compare with last sync-down.*.json
- [ ] SYNC_UP: Backup Google Sheet - Use Drive API files.copy endpoint
- [ ] SYNC_UP: Execute batch clear and write - Clear data ranges, write headers, write data
- [ ] SYNC_UP: Verification - Re-fetch row counts

## Db Interface Design Decisions

This section documents design decisions made for the `Db` struct's data operations. Decisions are
labeled:

- **CHANGE**: Modifies existing code or design
- **NEW**: Adds to the design without changing existing behavior

### Db Method Signatures

**NEW** - The `Db` struct will expose these public methods for data operations:

```rust
impl Db {
    /// Saves data from TillerData into the database.
    /// - Transactions: upsert (insert new, update existing, delete removed)
    /// - Categories: delete all, then insert all
    /// - AutoCat: delete all, then insert all
    pub async fn save_tiller_data(&self, data: &TillerData) -> Result<()>;

    /// Retrieves all data from the database as TillerData.
    /// Reconstructs Mapping from sheet_metadata table.
    pub async fn get_tiller_data(&self) -> Result<TillerData>;
}
```

- Affects: `src/db/mod.rs` (`Db` struct)
- Input/Output type: `TillerData` from `src/model/mod.rs:19-27`

### Transaction Sync Semantics

**NEW** - Transaction sync is a full sync operation:

1. Insert new transactions (by `transaction_id`)
2. Update existing transactions that have changed
3. Delete transactions that exist in DB but not in incoming data

This differs from categories/autocat which use simple delete-all + insert-all.

- Affects: `src/db/mod.rs` (new `save_tiller_data` method)
- Related: `Transaction` struct at `src/model/transaction.rs:94-121`

### Database Transaction Semantics

**NEW** - All sync operations run within a single SQLite transaction with rollback on error. If any
operation fails, the entire sync is rolled back and the database remains unchanged.

### Schema Changes Required (changes Migration 1)

#### NEW: `sheet_metadata` Table

Stores column ordering and header-to-column mapping for each sheet:

```sql
CREATE TABLE sheet_metadata
(
    sheet       TEXT    NOT NULL, -- 'transactions' | 'categories' | 'autocat'
    column_name TEXT    NOT NULL, -- snake_case SQLite column name (e.g., 'account_number')
    header_name TEXT    NOT NULL, -- original Google Sheet header (e.g., 'Account #')
    "order"     INTEGER NOT NULL, -- position in sheet (0-indexed)
    PRIMARY KEY (sheet, "order"),
    UNIQUE (sheet, header_name)
);
```

This table stores ALL columns including "other" (unknown) columns, enabling reconstruction of the
`Mapping` struct from the database.

- Affects: `src/db/migrations/migration_01_up.sql`
- Related: `Mapping` struct at `src/model/mapping.rs:21-27`
- Related: DESIGN.md lines 549-559 (Migration Files section)

#### CHANGE: Add `other_fields` Column to Data Tables

Each data table needs a TEXT column to store unknown/custom columns as JSON:

```sql
ALTER TABLE transactions
    ADD COLUMN other_fields TEXT;
ALTER TABLE categories
    ADD COLUMN other_fields TEXT;
ALTER TABLE autocat
    ADD COLUMN other_fields TEXT;
```

The JSON is keyed by **header name** (not snake_case column name) to match existing behavior in
`Transaction::set_with_header()` at `src/model/transaction.rs:182-184`:

```rust
Err(_) => {
let _ = self.other_fields.insert(header.to_string(), value);
}
```

Example JSON: `{"My Custom Column": "some value", "Another Column": "another value"}`

- Affects: `src/db/migrations/migration_01_up.sql`
- Related: `Transaction.other_fields` at `src/model/transaction.rs:120`
- Related: `Category.other_fields` at `src/model/category.rs:90`
- Related: `AutoCat.other_fields` at `src/model/auto_cat.rs:97`

#### NEW: Add `original_order` Column to Data Tables

Each data table needs an INTEGER column to track the original row position from sync down:

```sql
ALTER TABLE transactions
    ADD COLUMN original_order INTEGER;
ALTER TABLE categories
    ADD COLUMN original_order INTEGER;
ALTER TABLE autocat
    ADD COLUMN original_order INTEGER;
```

- Set during sync down to the row index from the sheet (0-indexed, excluding header)
- Locally-added rows have NULL
- Used for formula preservation and deletion detection

#### NEW: `formulas` Table

Stores cell formulas from the Google Sheet, keyed by absolute position:

```sql
CREATE TABLE formulas
(
    sheet   TEXT    NOT NULL, -- 'transactions' | 'categories' | 'autocat'
    row     INTEGER NOT NULL, -- 0-indexed row (excluding header)
    col     INTEGER NOT NULL, -- 0-indexed column
    formula TEXT    NOT NULL, -- the formula string (e.g., '=SUM(A1:A10)')
    PRIMARY KEY (sheet, row, col)
);
```

Formulas are tied to sheet positions, not to row data. During sync up, formulas are written back
to their original positions when `original_order` values indicate no deletions occurred.

- Affects: `src/db/migrations/migration_01_up.sql`
- Related: `Transactions.formulas` at `src/model/transaction.rs:17` (BTreeMap<RowCol, String>)

#### CHANGE: Categories Table Primary Key

The categories table uses a synthetic primary key to allow renaming categories:

```sql
CREATE TABLE categories
(
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    category          TEXT NOT NULL UNIQUE,
    category_group    TEXT,
    type              TEXT,
    hide_from_reports TEXT
);
```

The `category` column has a UNIQUE constraint but is not the primary key. This allows updating
the category name while maintaining referential integrity via the synthetic `id`.

- Affects: `src/db/migrations/migration_01_up.sql`
- Related: `Category` struct at `src/model/category.rs`

### Formula Preservation Strategy

Formula preservation is opt-in via the `--preserve-formulas` flag on `tiller sync up` (defaults to
false). When not set, formulas are ignored entirely.

**During sync down:**

- Each row's `original_order` is set to its row index from the sheet
- Cell formulas are captured and stored in the `formulas` table

**During sync up with `--preserve-formulas`:**

- Sort order: `original_order ASC NULLS LAST`, then by ID (e.g., `transaction_id`) for determinism
- New rows (NULL `original_order`) are appended at the end
- Formulas are written back to their original cell positions

**Deletion detection algorithm:**

1. Query non-NULL `original_order` values, sorted ascending
2. Iterate expecting sequential values: 0, 1, 2, 3...
3. If any gap exists (e.g., 0, 1, 3), at least one deletion occurred

**Sync up behavior when `--preserve-formulas` is set:**

| Condition                             | Behavior                                      |
|---------------------------------------|-----------------------------------------------|
| No deletions detected                 | Proceed, write formulas to original positions |
| Deletions detected (gaps in sequence) | ERROR, require `--force` flag                 |
| `--force` with deletions              | Write values only, skip all formulas          |

- Affects: `src/commands/sync.rs` (sync up logic)
- Related: `Transactions.formulas` at `src/model/transaction.rs` (BTreeMap<RowCol, String>)

### Existing Code Observations

**Uniqueness enforcement already exists** - The `Mapping::new()` method at
`src/model/mapping.rs:57-66` already enforces unique headers:

```rust
if header_map.len() != expected_length {
return Err(MappingError(String::from("Encountered a duplicate header")));
}
```

This satisfies the unique constraint requirement for `sheet_metadata(sheet, header_name)`.

**`count_transactions()` is a stub** - The method at `src/db/mod.rs:85-88` currently returns
hardcoded 100. This needs to be implemented to return the actual row count.

### Open Questions (Deferred)

The following question was raised but not resolved:

- For `count_transactions()`: Should it count all rows, or only rows with non-NULL `original_order`?
  (Used for precondition checks like "error if database is empty")

## Instruction Imports

- @./docs/ai/CHANGELOG_INSTRUCTIONS.md: Instructions for managing CHANGELOG.md following Keep a
  Changelog specification
- @./docs/ai/MARKDOWN.md: Instructions for formatting Markdown

The directory @./docs/ai contains Markdown files that provide additional instructions.

When the user asks you to define a new set of instructions, you should inquire whether the user
wants them added to this instruction file, or to a separate file in `docs/ai`. If the user wants a
separate instructions file, then you should create it in `docs/ai` and add an import of it here.

For example, let's say the user wants to add some instructions that are specifically about adding
Python code to this project. You ask the user, "Do you want these instructions added to this
instructions file, or do you want a separate file for these instructions?"

If the user says they want a separate file, you would then create a file at `docs/ai/PYTHON.md` and
add a line like the following below:

```markdown
- @./docs/ai/PYTHON.md: Instructions for writing, running and interacting with python code in this
  project.
```

## Rust Guidelines

NEVER use `unwrap`, `expect` or any other functions that can explicitly panic in production code (
it's fine in test code only, NEVER in production code).

## Build and Development Commands

When writing or editing Rust code, always run the following commands before you report that you are
done:

- Run `cargo fmt`
- Run `cargo clippy --all-features -- -D warnings` and fix all problems if possible

When editing code, make sure you run the tests:

- `cargo test`

You can quickly check your syntax without compiling with:

- `cargo check`

## Prohibited Dependencies

**NEVER add the following crates to this project:**

- `yup-oauth2` - We implement OAuth manually using the `oauth2` crate for full control over the user
  authentication flow
- `google-sheets4` - This crate is tightly coupled to `yup-oauth2` and lacks the control we need
  over
  the OAuth interaction

**Rationale:** These libraries do not provide sufficient control over when and how the OAuth browser
interaction occurs.

**ALWAYS ASK THE USER ABOUT NEW DEPENDENCIES**: When you are considering adding a new dependency to
Cargo.toml, ask the user first.
