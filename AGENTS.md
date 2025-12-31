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

Glossary:

- GENERAL: general steps related to design, quality or other non-specific taks
- SYNC_DOWN: work related to `tiller sync down` logic
- SYNC_UP: work related to `tiller sync up` logic
- BACKUP: work related to backup logic
- SQL: work related to the SQLite datastore

### Next Steps:

#### MCP Implementation:

- [X] Update DESIGN.md with MCP section
- [X] Migrate from `log`/`env_logger` to `tracing`/`tracing-subscriber`
- [X] Create public error type in `error.rs` supporting `isError` pattern
- [ ] Add `rmcp` dependency to Cargo.toml
- [ ] Add `tiller mcp` subcommand to CLI args
- [ ] Create `src/mcp/mod.rs` module structure
- [ ] Implement MCP server with stdio transport
- [ ] Write integration test for stdio transport
- [ ] Implement `sync_down` tool wrapper
- [ ] Write unit tests for MCP `sync_down` tool
- [ ] Add MCP logging notifications for important messages
- [ ] Implement `sync_up` tool wrapper (with `force`, `formulas` params)
- [ ] Write unit tests for MCP `sync_up` tool
- [ ] Write unit tests for MCP handlers

#### Further Development

- [ ] Design and implement an interface, both CLI and MCP, for querying, updating, deleting and
  inserting records

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
- Run `cargo clippy -- -D warnings && cargo clippy --all-features -- -D warnings` and fix all
  problems if possible

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
