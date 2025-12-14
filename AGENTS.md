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

Currently, the SQLite storage layer has not been started. We are not ready to begin that work. First
we are going to get `tiller sync up` and `tiller sync down` working using temporary or in-memory
persistence. Basically we will the Sheets API side code written first, then turn to SQLite code.

What's Already Built for `tiller sync up`

1. Data Download (sync down) - Complete
2. Model Layer - Complete
3. API Layer - Partial
    - Sheet trait has get(), get_formulas() (implemented) and _put() (stub with todo!())
    - GoogleSheet implements these for real Google Sheets API calls
    - TestSheet provides in-memory test data
    - Tiller trait has get_data() only - no put_data() method yet
4. OAuth & Config - Complete

Next Steps:

- [X] Investigate the existing code base to understand where we left off.
- [X] Precondition checks - Verify datastore exists with transactions
- [X] Backup SQLite - Not applicable yet (using in-memory/temp storage) DEFERRED
- [ ] Add sync-down.*.json backup logic to `tiller sync down`
- [ ] Download current sheet state - Save to sync-up-pre.*.json backup
- [ ] Conflict detection - Compare with last sync-down.*.json
- [ ] Build output data - Convert model objects to Vec<Vec<String>>
- [ ] Backup Google Sheet - Use Drive API files.copy endpoint
- [ ] Execute batch clear and write - Clear data ranges, write headers, write data
- [ ] Verification - Re-fetch row counts

## Instruction Imports

- @./docs/ai/CHANGELOG_INSTRUCTIONS.md: Instructions for managing CHANGELOG.md following Keep a
  Changelog specification
- @./docs/ai/MARKDOWN.md: Instructions for formatting Markdown

The directory @./docs/ai contains Markdown files that provide additional instructions.

When the user asks you to define a new set of instructions, you should inquire whether the user
wants them added to this instruction file, or to a separate file in `docs/ai`. If the user wants a
separate instructions file, then you should create it in `docs/ai` and add an import of it here.

For example, let's say the user wants to add some instructions that are specifically about adding
Python code to this project. You ask the user, "Do you want these instructions added to the
this instructions file, or do you want a separate file for these instructions?"

If the user says they want a separate file, you would then create a file at `docs/ai/PYTHON.md` and
add a line like the following below:

```markdown
- @./docs/ai/PYTHON.md: Instructions for writing, running and interacting with python code in this
  project.
```

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
