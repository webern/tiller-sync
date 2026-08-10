//! Making the `Transaction ID` column usable as a primary key.
//!
//! `transactions.transaction_id` is the primary key of the local datastore, but real Tiller sheets
//! routinely contain rows whose `Transaction ID` cannot serve as one:
//!
//! - **Blank.** Some feeds supply no ID at all. Apple Card is the well-known example, and it can
//!   account for hundreds of rows in an ordinary sheet.
//! - **Duplicated.** Malformed split markers such as `split:[1]` and `split:[2]` appear verbatim in
//!   the column, on every split row, instead of referencing the parent transaction's ID.
//!
//! Neither case means the row is corrupt, so refusing to sync would make the tool unusable against
//! an ordinary sheet. Instead, every affected row is given a surrogate `user-` ID and the sheet's
//! own value is kept in `original_transaction_id`, which is what gets written back on sync up. The
//! round trip therefore leaves the Transactions tab byte-for-byte unchanged.
//!
//! See <https://github.com/webern/tiller-sync/issues/37>.

use crate::model::{Transaction, Transactions};
use std::collections::{BTreeMap, BTreeSet};
use tracing::{debug, warn};

/// The most offending rows to name individually before summarizing the rest.
const MAX_REPORTED_ROWS: usize = 10;

/// Why a row's `Transaction ID` could not be used as a primary key.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum IdProblem {
    /// The column was empty.
    Blank,
    /// More than one row carried this value.
    Duplicate,
}

impl IdProblem {
    fn describe(&self) -> &'static str {
        match self {
            IdProblem::Blank => "blank",
            IdProblem::Duplicate => "duplicated",
        }
    }
}

/// One row that was given a surrogate ID.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ResolvedId {
    /// The row's position in the sheet, 1-based and counting the header, so it matches what the
    /// user sees in Google Sheets.
    pub(crate) sheet_row: u64,
    /// The value that was in the `Transaction ID` column.
    pub(crate) original: String,
    /// The surrogate now used as the primary key.
    pub(crate) surrogate: String,
    /// Why a surrogate was needed.
    pub(crate) problem: IdProblem,
}

/// What [`resolve_transaction_ids`] had to change.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct IdReport {
    pub(crate) resolved: Vec<ResolvedId>,
}

impl IdReport {
    pub(crate) fn is_empty(&self) -> bool {
        self.resolved.is_empty()
    }

    fn count(&self, problem: IdProblem) -> usize {
        self.resolved
            .iter()
            .filter(|r| r.problem == problem)
            .count()
    }

    /// Tells the user what happened.
    ///
    /// The whole point of this over the old behavior is that the user learns about *every* offending
    /// row rather than whichever one SQLite happened to reject first. The first few are named
    /// individually and the rest are counted, with a pointer at the query that lists them all.
    pub(crate) fn log(&self) {
        if self.is_empty() {
            return;
        }

        let blank = self.count(IdProblem::Blank);
        let duplicate = self.count(IdProblem::Duplicate);

        warn!(
            "{} of the sheet's rows have a Transaction ID that cannot be used as a key \
             ({blank} blank, {duplicate} duplicated). Each was given a surrogate `user-` ID for \
             local use. The sheet's own value is preserved and written back unchanged on sync up. \
             To list them: SELECT original_transaction_id, transaction_id, date, description FROM \
             transactions WHERE original_transaction_id IS NOT NULL",
            self.resolved.len()
        );

        for resolved in self.resolved.iter().take(MAX_REPORTED_ROWS) {
            debug!(
                "Sheet row {}: {} Transaction ID {:?} -> {}",
                resolved.sheet_row,
                resolved.problem.describe(),
                resolved.original,
                resolved.surrogate
            );
        }

        if self.resolved.len() > MAX_REPORTED_ROWS {
            debug!(
                "...and {} more rows with an unusable Transaction ID",
                self.resolved.len() - MAX_REPORTED_ROWS
            );
        }
    }
}

/// Gives a surrogate `user-` ID to every row whose `Transaction ID` cannot be a primary key,
/// returning a report of what changed.
///
/// A surrogate has to be **stable** across syncs. `sync down` upserts on `transaction_id`, so an ID
/// that changed from one sync to the next would make the row look deleted and re-added, discarding
/// any local edits on it. The surrogate is therefore derived from the row's own content rather than
/// from its position, which shifts whenever a row is added above it.
///
/// Rows that are identical in content would hash the same, so the occurrence number within a group
/// of identical rows is mixed in as well.
pub(crate) fn resolve_transaction_ids(transactions: &mut Transactions) -> IdReport {
    // Which sheet values appear more than once. A blank is handled separately: it is unusable even
    // when it appears only once.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for transaction in transactions.data() {
        *counts
            .entry(transaction.transaction_id.as_str())
            .or_default() += 1;
    }
    let duplicated: BTreeSet<String> = counts
        .into_iter()
        .filter(|(id, count)| *count > 1 && !id.trim().is_empty())
        .map(|(id, _)| id.to_string())
        .collect();

    let mut report = IdReport::default();
    let mut occurrences: BTreeMap<String, u32> = BTreeMap::new();

    for (index, transaction) in transactions.data_mut().iter_mut().enumerate() {
        let original = transaction.transaction_id.clone();
        let problem = if original.trim().is_empty() {
            IdProblem::Blank
        } else if duplicated.contains(&original) {
            IdProblem::Duplicate
        } else {
            continue;
        };

        let fingerprint = fingerprint(transaction);
        let occurrence = occurrences.entry(fingerprint.clone()).or_default();
        let surrogate = surrogate_id(&fingerprint, *occurrence);
        *occurrence += 1;

        transaction.original_transaction_id = Some(original.clone());
        transaction.transaction_id = surrogate.clone();

        report.resolved.push(ResolvedId {
            // +2: the sheet is 1-based and the first row is the header.
            sheet_row: index as u64 + 2,
            original,
            surrogate,
            problem,
        });
    }

    report
}

/// The row content that a surrogate ID is derived from.
///
/// Only fields that come from the bank are used. Anything the user can edit locally, such as the
/// category or a note, is left out, because editing it would otherwise change the row's key and
/// make the row look like a different one on the next sync.
///
/// The amount contributes its numeric value rather than its `Display`, which carries the formatting
/// the value was parsed from — a dollar sign, thousands separators. Reformatting the Amount column
/// in the sheet does not change what the row is, so it must not change the row's key.
fn fingerprint(transaction: &Transaction) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        transaction
            .original_transaction_id
            .as_deref()
            .unwrap_or(&transaction.transaction_id),
        transaction.date,
        transaction.amount.value(),
        transaction.description,
        transaction.full_description,
        transaction.account,
        transaction.institution,
    )
}

/// Builds the surrogate ID for a fingerprint and its occurrence number.
///
/// The `user-` prefix is the project's existing marker for an ID that Tiller did not assign.
fn surrogate_id(fingerprint: &str, occurrence: u32) -> String {
    let hash = fnv1a_64(format!("{fingerprint}\u{1f}{occurrence}").as_bytes());
    format!("user-{hash:016x}")
}

/// FNV-1a, 64-bit.
///
/// Written out rather than taken from `std::hash::DefaultHasher` because these hashes are persisted
/// as primary keys: the standard hasher's algorithm is explicitly allowed to change between Rust
/// releases, which would silently re-key every affected row. This is a fixed, specified algorithm
/// that will produce the same output forever.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Transactions;

    const HEADERS: &[&str] = &[
        "Transaction ID",
        "Date",
        "Description",
        "Amount",
        "Account",
        "Institution",
    ];

    /// Builds a `Transactions` from `(id, date, description, amount)` tuples.
    fn transactions(rows: &[(&str, &str, &str, &str)]) -> Transactions {
        let mut sheet: Vec<Vec<String>> = vec![HEADERS.iter().map(|h| h.to_string()).collect()];
        for (id, date, description, amount) in rows {
            sheet.push(vec![
                id.to_string(),
                date.to_string(),
                description.to_string(),
                amount.to_string(),
                "Apple Card".to_string(),
                "Apple".to_string(),
            ]);
        }
        Transactions::parse(sheet, Vec::<Vec<String>>::new()).unwrap()
    }

    #[test]
    fn test_unique_ids_are_left_alone() {
        let mut txns = transactions(&[
            ("69112cec0a57f52108456b88", "2025-01-01", "One", "-1.00"),
            ("690edd882cac40d381f9e518", "2025-01-02", "Two", "-2.00"),
        ]);

        let report = resolve_transaction_ids(&mut txns);

        assert!(report.is_empty());
        assert_eq!(
            txns.data()[0].transaction_id,
            "69112cec0a57f52108456b88",
            "an ordinary ID should be untouched"
        );
        assert!(txns.data()[0].original_transaction_id.is_none());
    }

    #[test]
    fn test_blank_ids_get_surrogates() {
        let mut txns = transactions(&[
            ("", "2025-01-01", "Apple Card One", "-1.00"),
            ("", "2025-01-02", "Apple Card Two", "-2.00"),
        ]);

        let report = resolve_transaction_ids(&mut txns);

        assert_eq!(report.resolved.len(), 2);
        assert_eq!(report.count(IdProblem::Blank), 2);
        for transaction in txns.data() {
            assert!(transaction.transaction_id.starts_with("user-"));
            assert_eq!(transaction.original_transaction_id.as_deref(), Some(""));
        }
        assert_ne!(
            txns.data()[0].transaction_id,
            txns.data()[1].transaction_id,
            "two blank rows must not collide"
        );
    }

    #[test]
    fn test_duplicate_ids_get_surrogates() {
        let mut txns = transactions(&[
            ("split:[1]", "2025-01-01", "Split A", "-1.00"),
            ("split:[2]", "2025-01-01", "Split B", "-2.00"),
            ("split:[1]", "2025-02-01", "Split C", "-3.00"),
        ]);

        let report = resolve_transaction_ids(&mut txns);

        // Both rows carrying `split:[1]` are rewritten. `split:[2]` appears once, so it stays.
        assert_eq!(report.resolved.len(), 2);
        assert_eq!(report.count(IdProblem::Duplicate), 2);
        assert_eq!(txns.data()[1].transaction_id, "split:[2]");
        assert_eq!(
            txns.data()[0].original_transaction_id.as_deref(),
            Some("split:[1]")
        );
        assert_eq!(
            txns.data()[2].original_transaction_id.as_deref(),
            Some("split:[1]")
        );
        assert_ne!(txns.data()[0].transaction_id, txns.data()[2].transaction_id);
    }

    /// Identical rows are plausible: the same card, the same merchant, the same amount, the same
    /// day. They still need distinct keys.
    #[test]
    fn test_identical_rows_get_distinct_surrogates() {
        let mut txns = transactions(&[
            ("", "2025-01-01", "Coffee", "-4.50"),
            ("", "2025-01-01", "Coffee", "-4.50"),
            ("", "2025-01-01", "Coffee", "-4.50"),
        ]);

        resolve_transaction_ids(&mut txns);

        let ids: std::collections::BTreeSet<&str> = txns
            .data()
            .iter()
            .map(|t| t.transaction_id.as_str())
            .collect();
        assert_eq!(
            ids.len(),
            3,
            "three identical rows need three distinct keys"
        );
    }

    /// A surrogate is a primary key, so it has to survive a re-sync. If it were derived from the
    /// row's position, inserting a row above it would re-key it and the upsert would treat the row
    /// as deleted and re-added, losing any local edits.
    #[test]
    fn test_surrogates_are_stable_when_rows_shift() {
        let row = ("", "2025-01-01", "Coffee", "-4.50");
        let mut before = transactions(&[row]);
        let mut after = transactions(&[("newer", "2025-06-01", "Newer row", "-9.99"), row]);

        resolve_transaction_ids(&mut before);
        resolve_transaction_ids(&mut after);

        assert_eq!(
            before.data()[0].transaction_id,
            after.data()[1].transaction_id,
            "the same row should keep its surrogate after another row is inserted above it"
        );
    }

    /// The surrogate must not depend on anything the user edits locally, or an edit would re-key
    /// the row.
    #[test]
    fn test_surrogates_ignore_locally_editable_fields() {
        let mut txns = transactions(&[("", "2025-01-01", "Coffee", "-4.50")]);
        resolve_transaction_ids(&mut txns);
        let original_key = txns.data()[0].transaction_id.clone();

        let mut edited = transactions(&[("", "2025-01-01", "Coffee", "-4.50")]);
        edited.data_mut()[0].category = "Coffee Shops".to_string();
        edited.data_mut()[0].note = "reimbursable".to_string();
        resolve_transaction_ids(&mut edited);

        assert_eq!(
            original_key,
            edited.data()[0].transaction_id,
            "categorizing a row must not change its key"
        );
    }

    /// The whole approach only works if the sheet round-trips unchanged.
    #[test]
    fn test_original_value_is_written_back() {
        let mut txns = transactions(&[
            ("", "2025-01-01", "Apple Card", "-1.00"),
            ("split:[1]", "2025-01-02", "Split A", "-2.00"),
            ("split:[1]", "2025-01-03", "Split B", "-3.00"),
        ]);
        let before = txns.to_rows().unwrap();

        resolve_transaction_ids(&mut txns);
        let after = txns.to_rows().unwrap();

        assert_eq!(
            before, after,
            "the rows written back to the sheet must be identical to the rows read from it"
        );
    }

    #[test]
    fn test_report_names_the_sheet_row() {
        let mut txns = transactions(&[
            ("good-id", "2025-01-01", "One", "-1.00"),
            ("", "2025-01-02", "Two", "-2.00"),
        ]);

        let report = resolve_transaction_ids(&mut txns);

        assert_eq!(report.resolved.len(), 1);
        // Data row index 1 is the third line of the sheet: header, good-id, then this one.
        assert_eq!(report.resolved[0].sheet_row, 3);
        assert_eq!(report.resolved[0].problem, IdProblem::Blank);
    }

    /// The hash is persisted as a primary key, so its output is part of the on-disk format.
    #[test]
    fn test_fnv1a_matches_the_published_vectors() {
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
