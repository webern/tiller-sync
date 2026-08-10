-- Migration 03 (down): Remove the original_transaction_id column
--
-- Rows that were given a surrogate ID keep it, but the sheet's own value is lost, so a `sync up`
-- after this downgrade would write surrogate IDs into the Transactions tab. Run `sync down` again
-- after downgrading to restore the sheet's values.

ALTER TABLE transactions
    DROP COLUMN original_transaction_id;
