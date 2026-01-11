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

The software is currently feature-complete. Here are the status updates from when it was being build
signifying the order in which subsystems were constructed:

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
9. Query operations are available in both CLI and MCP interfaces.
10. One or more releases has been published.

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
