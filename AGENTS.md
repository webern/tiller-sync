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
- [ ] SQL: STOP HERE: We need to design the interface and logic for Upserting data and querying data
  with Db
- [ ] SQL: REPLACE THIS STEP WITH THE IMPLEMENTATION PLAN for upserting and queries
- [ ] SYNC_UP: Build output data - Convert model objects to `Vec<Vec<String>>`
- [ ] SYNC_UP: Conflict detection - Compare with last sync-down.*.json
- [ ] SYNC_UP: Backup Google Sheet - Use Drive API files.copy endpoint
- [ ] SYNC_UP: Execute batch clear and write - Clear data ranges, write headers, write data
- [ ] SYNC_UP: Verification - Re-fetch row counts

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
