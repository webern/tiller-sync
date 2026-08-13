# Tiller Sync

[Tiller](https://tiller.com/) aggregates and categorizes financial transactions into a Google Sheet.
This server syncs that sheet with a local SQLite database, where the data can be queried and edited,
and syncs changes back.

Use it for anything about the user's spending, budget, transactions, categories, or AutoCat rules.

## Tools

- `sync_down` — download the sheet into the local database.
- `sync_up` — upload the local database to the sheet.
- `query` / `schema` — read-only SQL against the local database, and its structure.
- `insert_*`, `update_*`, `delete_*` — edit transactions, categories, and AutoCat rules locally.
- `instructions` — the in-depth guide, if you want more than each tool's own documentation.

## What to know before you start

- **Local edits are not automatically synced.** Nothing reaches the Google Sheet until `sync_up`.
- **`sync_down` overwrites local changes.** Run it before *starting* a round of edits, not between
  editing and `sync_up`. See `instructions` for how to check for remote changes without it.
- **`force` and `formulas` on `sync_up` exist to prevent data loss.** Read `sync_up`'s own
  documentation before setting either.
- Setup (`tiller init` and `tiller auth`) happens on the command line, not through this server. If
  the tools report an authentication error, ask the user to run `tiller auth` in a terminal.
