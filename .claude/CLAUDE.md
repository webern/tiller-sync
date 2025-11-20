# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this
repository.

- The project sourcecode is hosted on github at https://github.com/webern/tiller-sync.
- The project is published on crates.io at

## Project Overview

Tiller Sync is a Rust CLI tool for syncing data between a [Tiller](https://tiller.com/) Google Sheet
and a local SQLite database. The project is in early development stages with the basic CLI structure
in place.

## Build and Development Commands

```bash
# Build the project
cargo build

# Run the application
cargo run -- <command>

# Run tests
cargo test

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy for linting
cargo clippy
```

## Design and Architecture

See @../docs/DESIGN.md for design and architecture.

## External Instructions

The directory @../docs/ai contains Markdown files that provide additional instructions.

When the user asks you to define a new set of instructions, you should inquire whether the user
wants them added to this CLAUDE.md file, or to a separate file in `docs/ai`. If the user wants a
separate instructions file, then you should create it in `docs/ai` and add an import of it here.

For example, let's say the user wants to add some instructions that are specifically about adding
Python code to this project. You ask the user, "Do you want these instructions added to the
CLAUDE.md file, or do you want a separate file for these instructions?"

If the user says they want a separate file, you would then create a file at `docs/ai/PYTHON.md` and
add a line like the following below:

```markdown
- @../docs/ai/PYTHON.md: Instructions for writing, running and interacting with python code in this
  project.
```

### Instruction Imports

- @../docs/ai/CHANGELOG_INSTRUCTIONS.md: Instructions for managing CHANGELOG.md following Keep a
  Changelog specification
- @../docs/ai/MARKDOWN.md: Instructions for formatting Markdown

## Rust Instructions

When writing or editing Rust code, always run the following commands before you report that you are
done:

- Run `cargo fmt`
- Run `cargo clippy --all-features -- -D warnings` and fix all problems

## Prohibited Dependencies

**NEVER add the following crates to this project:**

- `yup-oauth2` - We implement OAuth manually using the `oauth2` crate for full control over the user
  authentication flow
- `google-sheets4` - This crate is tightly coupled to `yup-oauth2` and lacks the control we need over
  the OAuth interaction

**Rationale:** These libraries do not provide sufficient control over when and how the OAuth browser
interaction occurs. We need explicit control to ensure `tiller auth` is the only command that initiates
user authentication workflows, while other commands remain non-interactive and scriptable.
