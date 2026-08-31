-- Migration 03 (down): Re-key transactions on Tiller's `Transaction ID`.
--
-- The `transaction_id` values are prepared in Rust before this file runs (see `migration_03_down`
-- in `mod.rs`), which gives a usable value to every row whose `Transaction ID` is blank or shared
-- with another row. Those rows cannot round-trip through a schema that keys on that column, so the
-- downgrade puts the row's sync ID there instead.
--
-- Run `tiller sync down` after downgrading. The older schema cannot hold a sheet whose
-- `Transaction ID` column has blank or repeated values, which is most real Tiller sheets.

CREATE TABLE transactions_old
(
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
    category         TEXT REFERENCES categories (category) ON UPDATE CASCADE ON DELETE RESTRICT,
    note             TEXT,
    tags             TEXT,
    categorized_date TEXT,
    statement        TEXT,
    metadata         TEXT,
    original_order   INTEGER,
    other_fields     TEXT
);

INSERT INTO transactions_old (transaction_id, date, description, amount, account, account_number,
                              institution, account_id, month, week, full_description, check_number,
                              date_added, merchant_name, category_hint, category, note, tags,
                              categorized_date, statement, metadata, original_order, other_fields)
SELECT transaction_id,
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

ALTER TABLE transactions_old RENAME TO transactions;

CREATE INDEX idx_transactions_date ON transactions (date);
CREATE INDEX idx_transactions_account ON transactions (account);
CREATE INDEX idx_transactions_category ON transactions (category);
CREATE INDEX idx_transactions_description ON transactions (description);
