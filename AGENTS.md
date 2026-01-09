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

See @./docs/DESIGN.md for design and architecture. NOTE: when editing `DESIGN.md` remember that a
design doc is NOT a record of changes and decisions we have made along the way. The `DESIGN.md`
document **IS** a stateless record of the design as it currently stands. If explicitly asked to
preserve a design change, label it with **historical note**, but NEVER do this unless asked by the
user.

## Status and Progress: Current State

This section is here to help AI agents understand what we have been working on and what we need to
do next.

What's Already Built for `tiller sync up`

1. Data Download (`sync down`) - Complete
2. Model Layer - Complete
3. API Layer - Partial
    - Sheet trait has get(), get_formulas() (implemented) and `_put()` (stub with todo!())
    - GoogleSheet implements these for real Google Sheets API calls
    - TestSheet provides in-memory test data
    - Tiller trait has get_data() only - no put_data() method yet
4. OAuth & Config - Complete
5. Database migrations and initial schema creation
6. Data upload (`sync up`) - Implemented, working
7. An MCP server is instantiated and working and provides `sync_up` and `sync_down` tools.
8. Crud operations are available in both CLI and MCP interfaces.

### Next Steps: Query Interface Implementation

Implement a raw SQL query interface for AI agents (MCP) and CLI users. See `docs/DESIGN.md` for
full design details.

#### Implementation Guidelines

- At the end of each phase: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` must pass
- Use `#[expect(dead_code)]` for code not yet used (NEVER `#[allow(dead_code)]` or `_` prefixes)
- Do not include code in a phase if it cannot compile with `#[expect(dead_code)]`
- Commit at the end of each phase with a descriptive message

#### Implementation Plan

**Phase 1: Types and Args**

- [ ] Add `OutputFormat` enum to `src/args.rs` with variants `Json`, `Markdown`, `Csv`
- [ ] Add `QueryArgs` struct to `src/args.rs` with fields: `sql: String`, `format: Option<OutputFormat>`
- [ ] Add `SchemaArgs` struct to `src/args.rs` with field: `include_metadata: bool` (default false)
- [ ] Add `Query` and `Schema` variants to the CLI `Commands` enum in `src/main.rs`
- [ ] Verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
- [ ] Commit: `git commit -m "feat: add QueryArgs, SchemaArgs, and OutputFormat types"`

**Phase 2: Model Layer**

- [ ] Add `JsonSchema` as a supertrait bound on `Item` trait in `src/model/items.rs`
- [ ] Add `field_descriptions() -> BTreeMap<String, String>` associated function to `Item` trait
      with default implementation that extracts descriptions from `schemars::schema_for::<Self>()`
- [ ] Verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
- [ ] Commit: `git commit -m "feat: add field_descriptions() to Item trait via JsonSchema"`

**Phase 3: Command Layer**

- [ ] Create `src/commands/query.rs` with:
  - [ ] `Rows` enum: `Json(serde_json::Value)`, `Table(Vec<String>)`, `Csv(Vec<Vec<String>>)`
  - [ ] Implement `Debug`, `Display`, `Serialize`, `Deserialize`, `Clone` for `Rows`
  - [ ] `Schema` struct with `tables: Vec<TableInfo>` (derive `JsonSchema`)
  - [ ] `TableInfo`, `ColumnInfo`, `IndexInfo`, `ForeignKeyInfo` structs (all derive `JsonSchema`)
  - [ ] `pub async fn query(config: Config, args: QueryArgs) -> Result<Out<Rows>>`
  - [ ] `pub async fn schema(config: Config, args: SchemaArgs) -> Result<Out<Schema>>`
- [ ] Add `pub mod query;` to `src/commands/mod.rs`
- [ ] Verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
- [ ] Commit: `git commit -m "feat: add query and schema command implementations"`

**Phase 4: Database Layer**

- [ ] Modify `Db` struct in `src/db/mod.rs` to hold two pools:
  - `pool: SqlitePool` (read-write, existing)
  - `ro_pool: SqlitePool` (read-only, new)
- [ ] Update `Db::load()` and `Db::init()` to create both pools
  - Read-only pool uses `?mode=ro` in connection string
- [ ] Add `pub(crate) async fn execute_query(&self, args: QueryArgs) -> Res<Rows>`
  - Execute SQL on `ro_pool`
  - Convert results to appropriate `Rows` variant based on `args.format`
  - Count rows for message
- [ ] Add `pub(crate) async fn get_schema(&self, args: SchemaArgs) -> Res<Schema>`
  - Query `sqlite_master` for tables
  - Query `PRAGMA table_info()` for columns
  - Query `PRAGMA index_list()` and `PRAGMA index_info()` for indexes
  - Query `PRAGMA foreign_key_list()` for foreign keys
  - Get row counts with `SELECT COUNT(*) FROM <table>`
  - Get column descriptions from `Item::field_descriptions()` for data tables
  - Filter tables based on `include_metadata`
- [ ] Verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
- [ ] Commit: `git commit -m "feat: add execute_query and get_schema to Db with read-only pool"`

**Phase 5: MCP Tools**

- [ ] Add `query` tool to `src/mcp/tools.rs`:
  - `#[tool]` with detailed doc comment explaining raw SQL, read-only, large result warning
  - Parameters: `sql` (required), `format` (required)
  - Returns `Rows` via `tool_result()`
- [ ] Add `schema` tool to `src/mcp/tools.rs`:
  - `#[tool]` with doc comment explaining schema structure
  - Parameters: `include_metadata` (optional, default false)
  - Returns `Schema` via `tool_result()`
- [ ] Verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
- [ ] Commit: `git commit -m "feat: add query and schema MCP tools"`

**Phase 6: Documentation**

- [ ] Add brief mention of `query` and `schema` tools to `src/mcp/docs/INSTRUCTIONS.md`
- [ ] Verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
- [ ] Commit: `git commit -m "docs: add query and schema tools to MCP instructions"`

**Phase 7: Integration Testing**

- [ ] Manual testing of CLI commands:
  - `tiller query "SELECT * FROM transactions LIMIT 5"`
  - `tiller query --format markdown "SELECT * FROM categories"`
  - `tiller query --format csv "SELECT category, COUNT(*) FROM transactions GROUP BY category"`
  - `tiller schema`
  - `tiller schema --include-metadata`
- [ ] Manual testing of MCP tools via `tiller mcp`
- [ ] Verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
- [ ] Commit: `git commit -m "test: verify query interface integration"`

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

For dead code warnings during incremental development:
- Use `#[expect(dead_code)]` to silence warnings (this will error when the code becomes used)
- NEVER use `#[allow(dead_code)]`
- NEVER use underscore prefixes (e.g., `_foo`) to silence dead code warnings

### MCP `rmpc` and `schemars` Descriptions

- ALWAYS write descriptions in doc comments.
- NEVER use the `description` field of `rmpc` derive macros (such as the `tool`, `schemars`, or
  `JsonSchema` macros).

### MCP Server Documentation Files

The MCP server has two important documentation files in `src/mcp/docs/`:

- **`INTRO.md`**: Brief description shown to the AI client upon MCP server initialization (via
  `ServerInfo.instructions`). Keep this concise.
- **`INSTRUCTIONS.md`**: In-depth usage guide that the agent must read before using tools. This is
  returned by the `initialize_service` tool and contains detailed information about workflows,
  parameters, and best practices.

### Restrictive use of Pub

`pub` functions in this library should be restricted to these modules: `commands`, `args`, `model`
and `error`. Additionally, types taken as arguments of `pub` functions or returned by `pub`
functions need to also be `public`.

NEVER: change something from `private`, `pub(crate)` or `pub(super)` to `pub` without asking the
user if it is Ok to do so.

ALWAYS: default to `private` or `pub(crate)` when you are not sure if something needs to be part of
the public interface.

## Build and Development Commands

When writing or editing Rust code, always run the following commands before you report that you are
done:

- Run `cargo fmt`
- Run `cargo clippy -- -D warnings && cargo clippy --all-features -- -D warnings` and fix all
  problems if possible

When editing code, make sure you run the tests:

- `cargo test`

You can quickly check your syntax without compiling with:

- `cargo check`

## Dependencies

**ALWAYS ASK THE USER ABOUT NEW DEPENDENCIES**: When you are considering adding a new dependency to
Cargo.toml, ask the user first.
