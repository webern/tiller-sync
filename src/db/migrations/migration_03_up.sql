-- Migration 03: Record the sheet's own Transaction ID when it cannot serve as a primary key
--
-- Real Tiller sheets contain rows whose Transaction ID is blank (some feeds, notably Apple Card,
-- supply no IDs) or duplicated (malformed split markers such as `split:[1]`). Those values cannot
-- be a primary key, so `sync down` assigns a surrogate `user-` ID to such rows.
--
-- `original_transaction_id` holds the sheet's value verbatim so `sync up` can write it back
-- unchanged. It is NULL for the ordinary case, where `transaction_id` is the sheet's own value.

ALTER TABLE transactions
    ADD COLUMN original_transaction_id TEXT;
