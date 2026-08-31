-- Migration 03: Re-key transactions on the synthetic sync ID.
--
-- Tiller's `Transaction ID` cannot be a primary key: some feeds supply no value at all, and split
-- markers such as `split:[1]` repeat across rows. The column is demoted to ordinary data and the
-- table is keyed on `sync_id`, the identifier this tool assigns and owns.
--
-- The `sync_id` values are computed in Rust before this file runs (see `migration_03_up` in
-- `mod.rs`), which populates the `sync_id` column added by the first statement below. This file
-- performs the table rebuild that SQLite needs in order to change a primary key.

CREATE TABLE transactions_new
(
    sync_id          TEXT PRIMARY KEY,
    transaction_id   TEXT NOT NULL DEFAULT '',
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
    category         TEXT REFERENCES categories (category) ON UPDATE CASCADE ON DELETE RESTRICT,
    note             TEXT,
    tags             TEXT,
    categorized_date TEXT,
    statement        TEXT,
    metadata         TEXT,
    original_order   INTEGER,
    other_fields     TEXT
);

INSERT INTO transactions_new (sync_id, transaction_id, date, description, amount, account,
                              account_number, institution, account_id, month, week,
                              full_description, check_number, date_added, merchant_name,
                              category_hint, category, note, tags, categorized_date, statement,
                              metadata, original_order, other_fields)
SELECT sync_id,
       transaction_id,
       date,
       description,
       amount,
       account,
       account_number,
       institution,
       account_id,
       month,
       week,
       full_description,
       check_number,
       date_added,
       merchant_name,
       category_hint,
       category,
       note,
       tags,
       categorized_date,
       statement,
       metadata,
       original_order,
       other_fields
FROM transactions;

DROP TABLE transactions;

ALTER TABLE transactions_new RENAME TO transactions;

CREATE INDEX idx_transactions_date ON transactions (date);
CREATE INDEX idx_transactions_account ON transactions (account);
CREATE INDEX idx_transactions_category ON transactions (category);
CREATE INDEX idx_transactions_description ON transactions (description);
CREATE INDEX idx_transactions_transaction_id ON transactions (transaction_id);
