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

### Next Steps:

Next we need to develop the CLI/MCP interface for interacting with locally stored Transactions,
Categories, and AutoCats. Let's work on these one item at a time. NEVER do multiple items at a time.

- [x] Schema changes (Change migration 1: DO NOT add new migrations)
    - [x] Change categories table such that the category name field is the primary key
    - [x] Create a foreign key constraint between transactions and categories.
    - [x] Create a foreign key constraint between autocats and categories.
    - [x] Update documentation to note these foreign key constraints

- [ ] Update Transactions
    - [ ] Design and implement a CLI interface and command for updating a single transaction by ID
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Update Categories
    - [ ] Design and implement a CLI interface and command for updating a single category by ID
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Update AutoCats
    - [ ] Design and implement a CLI interface and command for updating a single autocat by ID
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Delete Transactions
    - [ ] Design and implement a CLI interface and command for deleting a single transaction by ID
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Delete Categories
    - [ ] Design and implement a CLI interface and command for deleting a single category by ID
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Delete AutoCats
    - [ ] Design and implement a CLI interface and command for deleting a single autocat by ID
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Insert Transactions
    - [ ] Design and implement a CLI interface and command for inserting a single transaction. NOTE:
      for transactions, a unique ID will need to be synthesized prior to table insert and that ID
      will need to be returned to the caller. NOTE: The category field is primary key constrained to
      the categories table.
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Insert Categories
    - [ ] Design and implement a CLI interface and command for inserting a single category. NOTE:
      for categories, the category name will need to be unique as it is the primary key.
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

- [ ] Insert AutoCats
    - [ ] Design and implement a CLI interface and command for inserting a single autocat. NOTE: The
      category field is primary key constrained to the categories table. NOTE: the primary key is
      auto-generated and needs to be returned to the caller.
    - [ ] Test the command
    - [ ] Implement an MCP server for the same command

STOP HERE: This part is hard to design. We need to think about how to provide a robust query
interface that presents all the things a user might want to do with a SQL statement. For this
design, consider the following MCP use-cases:

- A user wants to use an AI agent to suggest auto-cat rules for transactions that do not have an
  assigned category. These should be prioritized by frequency of the un-categorized transactions'
  description fields.
- A user wants to categorize and tag transactions differently than they are in the category field,
  or to assign some additional attributes to the transactions then have the LLM do an analysis on
  the sums.

- [ ] Query Transactions
- [ ] Query Categories
- [ ] Query AutoCats
- [ ] Query to get all data

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

PREFER using an underscore to silence dead_code warnings. Do not use `#[allow(dead_code)]`.

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
