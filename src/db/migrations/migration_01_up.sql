-- Migration 01: Create transactions, categories, and autocat tables

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
    category         TEXT,
    note             TEXT,
    tags             TEXT,
    categorized_date TEXT,
    statement        TEXT,
    metadata         TEXT
);

CREATE INDEX idx_transactions_date ON transactions (date);
CREATE INDEX idx_transactions_account ON transactions (account);
CREATE INDEX idx_transactions_category ON transactions (category);
CREATE INDEX idx_transactions_description ON transactions (description);

CREATE TABLE categories (
    category          TEXT PRIMARY KEY,
    category_group    TEXT,
    type              TEXT,
    hide_from_reports TEXT
);

CREATE TABLE autocat (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT,
    category                  TEXT,
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
    amount_contains           TEXT
);
