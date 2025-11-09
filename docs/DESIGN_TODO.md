# Design Documentation TODO

This document tracks missing sections and design decisions needed to complete the Tiller design
documentation.

## High Priority (Critical for MVP)

### Database Schema

- [X] **Transactions table schema** - Define all columns, types, constraints, and indexes for the
  Transactions table
    - Map to Tiller sheet columns from https://help.tiller.com/en/articles/432681
    - Primary key: Transaction ID
    - Required vs optional fields
    - Data types (TEXT, REAL, INTEGER, DATE)
    - Indexes for common queries (date, category, account)

- [ ] **Categories table schema** - Define structure for Categories
    - What columns exist? Just category names or more?
    - How are categories stored in Tiller sheet?
    - Primary key design
    - Parent/child relationships (if hierarchical)?

- [ ] **AutoCat table schema** - Define structure for AutoCat rules
    - What is AutoCat? (automatic categorization rules?)
    - How do rules work?
    - Schema for storing rules

### CLI Command Reference

- [ ] **Complete command tree** - Document all commands and subcommands
    ```
    tiller
    ├── auth
    ├── init (?)
    ├── sync
    │   ├── up
    │   └── down
    ├── query
    │   ├── transactions
    │   ├── categories
    │   └── autocat (?)
    ├── add (?)
    │   └── transaction
    ├── edit (?)
    │   └── transaction
    └── mcp
    ```

- [ ] **Common flags** - Document global flags available on all commands
    - `--dir` / `TILLER_HOME`
    - `--verbose` / `-v`
    - `--quiet` / `-q`
    - `--format` (json, csv, table)?

- [ ] **Output formats** - Specify output format for each command
    - Human-readable table format
    - JSON for scripting
    - CSV for export

- [ ] **Usage examples** - Provide real-world usage examples for each command

### MCP Tools Specification

- [ ] **Complete MCP tool listing** - List all MCP tools with signatures
    - `tiller__sync_up`
    - `tiller__sync_down`
    - `tiller__query_transactions`
    - `tiller__add_transaction`
    - `tiller__update_transaction`
    - `tiller__delete_transaction`
    - `tiller__get_categories`
    - `tiller__get_summary`
    - Others?

- [ ] **Tool parameter schemas** - Define JSON schema for each tool's parameters

- [ ] **Tool response schemas** - Define JSON schema for each tool's response

- [ ] **Error response format** - Standard error format for MCP tools

### Conflict Resolution Strategy

- [ ] **Conflict detection** - How to detect conflicts between local and remote
    - Use timestamps?
    - Use version numbers?
    - Compare hashes?

- [ ] **Conflict resolution for sync down** - Who wins when both modified?
    - Always prefer sheet (overwrite local)?
    - Detect and warn user?
    - Three-way merge?

- [ ] **Conflict resolution for sync up** - Who wins when both modified?
    - Always prefer local (overwrite sheet)?
    - Detect and warn user?

- [ ] **Deletion conflicts** - What if item deleted in one place, modified in other?

### Error Handling and Rollback

- [ ] **Network failure handling** - What happens if network fails mid-sync?
    - Partial sync state?
    - Rollback to last backup?
    - Resume from checkpoint?

- [ ] **OAuth token expiration** - Handle expired tokens during operations
    - Auto-refresh?
    - Graceful error with re-auth prompt?

- [ ] **Invalid sheet structure** - What if Google Sheet has wrong columns/tabs?
    - Validation before sync?
    - Helpful error messages?

- [ ] **Database corruption** - What if SQLite file is corrupted?
    - Restore from backup?
    - Re-sync from sheet?

- [ ] **Rollback strategy** - When to rollback and how?
    - Transaction support in SQLite?
    - Restore backup on failure?

### Data Types and Formats

- [ ] **Date format** - Specify date representation
    - ISO 8601? (YYYY-MM-DD)
    - Timezone handling?
    - How does Tiller store dates?

- [ ] **Amount/currency format** - Specify monetary value representation
    - Float vs Decimal?
    - How many decimal places?
    - Currency symbol handling?
    - Negative for expenses, positive for income?

- [ ] **Category names** - Constraints and validation
    - Max length?
    - Allowed characters?
    - Case sensitive?

- [ ] **Account names** - Constraints and validation
    - Max length?
    - Allowed characters?

- [ ] **Transaction descriptions** - Constraints
    - Max length?
    - Special character handling?

### Initialization and First-Run Flow

- [ ] **First-time setup** - Define initialization process
    - `tiller init` command?
    - Prompt for Google Sheet URL?
    - Create directory structure?
    - Run `auth` automatically?
    - Initial `sync down`?

- [ ] **Sheet validation** - Verify Google Sheet has correct structure
    - Check for required tabs (Transactions, Categories, AutoCat)?
    - Check for required columns?
    - Helpful error if wrong structure?

- [ ] **Config generation** - Auto-generate config.json
    - Prompt for settings?
    - Sensible defaults?

## Medium Priority (Important but can be refined later)

### Categories and AutoCat Details

- [ ] **Categories explanation** - What are Categories in Tiller?
    - Just a list of category names?
    - Hierarchical (parent/child)?
    - Custom fields?
    - How are they used in transactions?

- [ ] **AutoCat explanation** - What is AutoCat and how does it work?
    - Automatic categorization rules?
    - Pattern matching?
    - How to create/edit rules?
    - When are rules applied?

- [ ] **Category IDs** - Do Categories have IDs like Transactions?
    - If so, same scheme (Tiller vs local-)?
    - If not, how are they uniquely identified?

### Query Operations

- [ ] **Filtering capabilities** - What filters are supported?
    - Date range (start_date, end_date)
    - Category
    - Account
    - Amount range (min, max)
    - Description text search
    - Combination of filters (AND/OR)?

- [ ] **Sorting** - How can results be sorted?
    - By date (ascending/descending)
    - By amount
    - By description
    - Multiple sort keys?

- [ ] **Pagination** - How to handle large result sets?
    - Limit and offset?
    - Cursor-based pagination?
    - Default page size?

- [ ] **Aggregations** - Summary operations
    - Sum by category
    - Sum by account
    - Average transaction size
    - Transaction count
    - Date range grouping (by month, year)?

### Performance Considerations

- [ ] **Database indexes** - Which columns need indexes?
    - Date (for range queries)
    - Category (for filtering)
    - Account (for filtering)
    - Transaction ID (primary key, already indexed)

- [ ] **Caching strategy** - Should we cache anything?
    - Google Sheets API responses?
    - Parsed categories/accounts?
    - Query results?

- [ ] **Batch operations** - How to optimize bulk operations?
    - Batch inserts to SQLite?
    - Batch updates to Google Sheets?
    - Transaction boundaries?

### Transaction Field Mappings

- [ ] **Complete field mapping** - Map all Tiller columns to SQLite
    - Reference: https://help.tiller.com/en/articles/432681
    - Which fields are required vs optional?
    - Which fields are editable vs read-only?
    - Data type for each field

- [ ] **Custom fields** - Does Tiller support custom columns?
    - If so, how to handle in SQLite?
    - Dynamic schema?
    - Store as JSON blob?

## Low Priority (Nice to have)

### Testing Strategy

- [ ] **Unit testing approach** - How to test without real Google Sheets?
    - Mock Google Sheets API?
    - Test fixtures with sample data?
    - Property-based testing?

- [ ] **Integration testing** - End-to-end test strategy
    - Use test Google Sheet?
    - Temporary SQLite databases?
    - Automated test runs?

- [ ] **MCP testing** - How to test MCP mode?
    - Mock MCP client?
    - Test JSON-RPC messages?

### Schema Migration

- [ ] **Version management** - How to handle schema changes over time?
    - SQLite migrations?
    - What if Tiller adds new columns?
    - Backward compatibility?

- [ ] **Migration scripts** - Define migration process
    - Auto-migrate on startup?
    - Manual migration command?
    - Migration history tracking?

### Backup and Restoration

- [ ] **Backup rotation** - More details on backup management
    - Naming convention (already defined: YYYY-MM-DD-NNN)
    - Automatic cleanup (already defined: backup_copies)
    - Manual backup command?

- [ ] **Restore procedure** - How to restore from backup
    - `tiller restore --backup <file>` command?
    - List available backups?
    - Confirm before restoring?

- [ ] **Export/import** - Bulk data operations
    - Export to CSV?
    - Import from CSV?
    - Format specification?

### Advanced Features

- [ ] **Dry-run mode** - Preview changes before syncing
    - `--dry-run` flag for sync commands?
    - Show what would be added/modified/deleted?

- [ ] **Selective sync** - Sync only certain data
    - Date range sync?
    - Category filter?
    - Account filter?

- [ ] **Reports** - Built-in reporting
    - Monthly spending by category?
    - Account balances?
    - Trends over time?

- [ ] **Search** - Advanced search capabilities
    - Full-text search in descriptions?
    - Regular expression support?
    - Fuzzy matching?

## Design Decisions Needed

- [ ] Should we support multiple Tiller sheets (multiple configs)?
- [ ] Should database support multiple users/profiles?
- [ ] Should we cache OAuth tokens in system keychain vs file?
- [ ] Should sync be bidirectional or unidirectional?
- [ ] Should we validate data before syncing (business rules)?
- [ ] Should we support plugins/extensions?
- [ ] Should CLI support interactive mode (REPL)?
