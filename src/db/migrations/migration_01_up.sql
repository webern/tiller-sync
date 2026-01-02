-- Migration 01: Create transactions, categories, and autocat tables
--
-- Foreign Key Design:
-- - categories.category is the primary key (category name is unique identifier)
-- - transactions.category references categories.category with ON UPDATE CASCADE
-- - autocat.category references categories.category with ON UPDATE CASCADE
-- - ON DELETE RESTRICT prevents deleting categories that are in use
--
-- Note: During bulk sync operations (sync down/up), foreign key constraints are
-- temporarily disabled using PRAGMA foreign_keys = OFF to allow efficient
-- delete-all-then-insert patterns.

-- Categories must be created first since transactions and autocat reference it
CREATE TABLE categories (
    category          TEXT PRIMARY KEY,
    category_group    TEXT,
    type              TEXT,
    hide_from_reports TEXT,
    original_order    INTEGER,
    other_fields      TEXT
);

CREATE TABLE transactions (
    transaction_id   TEXT PRIMARY KEY,
    date             TEXT NOT NULL,
    description      TEXT NOT NULL,
    amount           NUMERIC NOT NULL,
    account          TEXT NOT NULL,
    account_number   TEXT NOT NULL,
    institution      TEXT NOT NULL,
    account_id       TEXT NOT NULL,
    month            TEXT,
    week             TEXT,
    full_description TEXT,
    check_number     TEXT,
    date_added       TEXT,
    merchant_name    TEXT,
    category_hint    TEXT,
    category         TEXT REFERENCES categories(category) ON UPDATE CASCADE ON DELETE RESTRICT,
    note             TEXT,
    tags             TEXT,
    categorized_date TEXT,
    statement        TEXT,
    metadata         TEXT,
    original_order   INTEGER,
    other_fields     TEXT
);

CREATE INDEX idx_transactions_date ON transactions (date);
CREATE INDEX idx_transactions_account ON transactions (account);
CREATE INDEX idx_transactions_category ON transactions (category);
CREATE INDEX idx_transactions_description ON transactions (description);

CREATE TABLE autocat (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    category                  TEXT REFERENCES categories(category) ON UPDATE CASCADE ON DELETE RESTRICT,
    description               TEXT,
    description_contains      TEXT,
    account_contains          TEXT,
    institution_contains      TEXT,
    amount_min                TEXT,
    amount_max                TEXT,
    amount_equals             TEXT,
    description_equals        TEXT,
    description_full          TEXT,
    full_description_contains TEXT,
    amount_contains           TEXT,
    original_order            INTEGER,
    other_fields              TEXT
);

CREATE TABLE sheet_metadata (
    sheet       TEXT    NOT NULL,
    column_name TEXT    NOT NULL,
    header_name TEXT    NOT NULL,
    "order"     INTEGER NOT NULL,
    PRIMARY KEY (sheet, "order"),
    UNIQUE (sheet, header_name)
);

CREATE TABLE formulas (
    sheet   TEXT    NOT NULL,
    row     INTEGER NOT NULL,
    col     INTEGER NOT NULL,
    formula TEXT    NOT NULL,
    PRIMARY KEY (sheet, row, col)
);
