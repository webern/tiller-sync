//! The synthetic row identifier that this tool assigns and owns.
//!
//! Tiller's own `Transaction ID` column cannot serve as a key: some feeds supply no value at all,
//! and split markers such as `split:[1]` repeat across rows. Rows are therefore identified by a
//! sync ID, stored in the database as `transactions.sync_id` and in the Google Sheet in the column
//! headed [`SYNC_ID_STR`].
//!
//! This module holds the pure part of that scheme: minting, validation, and working out which
//! rows of a downloaded Transactions grid need an identifier. The download, write-back and
//! verification around it live in the `api` module.

use crate::error::Res;
pub(crate) use crate::model::transaction::SYNC_ID_STR;
use crate::model::transaction::{DATE_STR, TRANSACTION_ID_STR};
use anyhow::bail;
use std::collections::{BTreeMap, HashSet};

/// Every sync ID starts with this, whether it was minted or seeded from a `Transaction ID`.
///
/// The uniform prefix is what lets `sync down` tell its own identifiers apart from somebody
/// else's data sitting in a column that happens to share our header.
pub(crate) const SYNC_ID_PREFIX: &str = "sync-";

/// The maximum length of the part of a sync ID that follows [`SYNC_ID_PREFIX`].
const MAX_SUFFIX_LEN: usize = 64;

/// Mints a new sync ID.
///
/// The value is a UUIDv4 with the dashes removed and truncated to 19 characters, prefixed with
/// [`SYNC_ID_PREFIX`], giving identifiers like `sync-f47e8c2a9b3d4f1ea80`. It is random rather
/// than derived from the row's contents: an identifier computed from the date, amount or
/// description would change when the user edited the row, and a row whose identifier changes looks
/// deleted and re-added, taking any local-only work with it.
pub(crate) fn mint_sync_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    let hex = uuid.as_simple().to_string(); // 32 hex chars, no dashes
    format!("{SYNC_ID_PREFIX}{}", &hex[..19])
}

/// Mints a sync ID that is not already present in `taken`.
///
/// A collision between two UUIDv4 values is vanishingly unlikely, but the value is about to become
/// a primary key, so it is cheap to be certain.
pub(crate) fn mint_unique_sync_id(taken: &HashSet<String>) -> String {
    loop {
        let candidate = mint_sync_id();
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
}

/// Whether `value` has the shape of a sync ID this tool would have written.
///
/// This is what distinguishes our column from a pre-existing one that happens to carry the same
/// header. It is a check on shape only; it says nothing about whether the identifier is known.
pub(crate) fn is_well_formed(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix(SYNC_ID_PREFIX) else {
        return false;
    };
    !suffix.is_empty()
        && suffix.len() <= MAX_SUFFIX_LEN
        && suffix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Builds the sync ID that seeds a row from its Tiller `Transaction ID` during the bootstrap.
///
/// Returns `None` when the value cannot be used, in which case the row is minted instead. Seeding
/// keeps a sheet whose transaction IDs were already unique on the keys it already had, so nothing
/// is re-keyed by the upgrade.
pub(crate) fn seed_from_transaction_id(transaction_id: &str) -> Option<String> {
    let trimmed = transaction_id.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SUFFIX_LEN - SYNC_ID_PREFIX.len() {
        return None;
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    Some(format!("{SYNC_ID_PREFIX}{trimmed}"))
}

/// What the sync ID assignment phase of `sync down` intends to do to the Transactions tab.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SyncIdAssignment {
    /// The 0-based column index that holds, or will hold, the sync ID.
    column: usize,
    /// True when the column did not exist and its header has to be written.
    creates_column: bool,
    /// The sync ID for every data row, in sheet order.
    ids: Vec<String>,
    /// The data rows whose cell has to be written, as `(0-based data row, value)`.
    writes: Vec<(usize, String)>,
    /// How many rows were seeded from an existing `Transaction ID`.
    seeded: usize,
    /// How many rows were given a freshly minted identifier.
    minted: usize,
}

impl SyncIdAssignment {
    /// The 0-based column index that holds, or will hold, the sync ID.
    pub(crate) fn column(&self) -> usize {
        self.column
    }

    /// True when the column did not exist and its header has to be written.
    pub(crate) fn creates_column(&self) -> bool {
        self.creates_column
    }

    /// The sync ID for every data row, in sheet order.
    pub(crate) fn ids(&self) -> &[String] {
        &self.ids
    }

    /// The data rows whose cell has to be written, as `(0-based data row, value)`.
    pub(crate) fn writes(&self) -> &[(usize, String)] {
        &self.writes
    }

    /// How many rows were seeded from an existing `Transaction ID`.
    pub(crate) fn seeded(&self) -> usize {
        self.seeded
    }

    /// How many rows were given a freshly minted identifier.
    pub(crate) fn minted(&self) -> usize {
        self.minted
    }

    /// True when the sheet already holds every identifier and nothing needs to be written.
    ///
    /// This is the steady state, and it is what keeps an ordinary `sync down` read-only.
    pub(crate) fn is_noop(&self) -> bool {
        !self.creates_column && self.writes.is_empty()
    }
}

/// Works out the sync ID for every row of a downloaded Transactions grid.
///
/// `grid` is the tab exactly as fetched: the header row followed by the data rows, with rows
/// possibly shorter than the header when their trailing cells are empty. `known` holds the sync IDs
/// already present in the local database, so that a freshly minted identifier cannot collide with
/// one already in use.
///
/// Rows that already carry an identifier keep it untouched. Rows with a blank cell are minted. When
/// the column is absent altogether the bootstrap runs: the header is appended to the right of every
/// existing column and each row is seeded from its `Transaction ID` where that value is non-blank
/// and unique across the whole tab, and minted otherwise.
///
/// # Errors
///
/// - The column holds a value this tool would not have written, which means either a pre-existing
///   column of the user's that happens to share our header, or a cell the user has edited.
/// - The column repeats a value. Choosing which row keeps it is not safe, so the user has to say.
pub(crate) fn assign_sync_ids(
    grid: &[Vec<String>],
    known: &HashSet<String>,
) -> Res<SyncIdAssignment> {
    let Some(header) = grid.first() else {
        bail!("The Transactions tab is empty; it must at least have a header row");
    };

    let sync_col = header.iter().position(|h| h == SYNC_ID_STR);
    let creates_column = sync_col.is_none();
    let column = sync_col.unwrap_or(header.len());
    let rows = &grid[1..];

    let existing: Vec<&str> = rows.iter().map(|row| cell(row, column)).collect();

    check_well_formed(&existing)?;
    check_unique(&existing)?;

    // Seeding only happens on the bootstrap run. Afterwards a blank cell means a row that Tiller
    // or the user has added since the last `sync down`, and it is minted like any other new row.
    let seeds = if creates_column {
        seeds_by_row(header, rows)
    } else {
        BTreeMap::new()
    };

    // Identifiers already spoken for in this tab. A seed is checked against these, and only
    // these: seeding deliberately reuses the identity the datastore already holds for the row,
    // which is the whole point of the bootstrap, so a seed that matches a known identifier is a
    // match rather than a collision. It cannot be another row's, because the seed is derived from
    // a `Transaction ID` that is unique across the tab, by the same rule the migration used.
    let mut assigned: HashSet<String> = existing
        .iter()
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .collect();

    // Identifiers a minted value has to avoid, which additionally covers every row the datastore
    // holds, including rows no longer in the sheet.
    let mut taken: HashSet<String> = known.union(&assigned).cloned().collect();

    let mut ids = Vec::with_capacity(rows.len());
    let mut writes = Vec::new();
    let mut seeded = 0;
    let mut minted = 0;

    for (row_ix, current) in existing.iter().enumerate() {
        if !current.is_empty() {
            ids.push((*current).to_string());
            continue;
        }

        let id = match seeds.get(&row_ix) {
            Some(seed) if !assigned.contains(seed) => {
                seeded += 1;
                seed.clone()
            }
            _ => {
                minted += 1;
                mint_unique_sync_id(&taken)
            }
        };

        assigned.insert(id.clone());
        taken.insert(id.clone());
        writes.push((row_ix, id.clone()));
        ids.push(id);
    }

    Ok(SyncIdAssignment {
        column,
        creates_column,
        ids,
        writes,
        seeded,
        minted,
    })
}

/// Rejects any value in our column that this tool would not have written.
fn check_well_formed(existing: &[&str]) -> Res<()> {
    let offenders: Vec<String> = existing
        .iter()
        .enumerate()
        .filter(|(_, value)| !value.is_empty() && !is_well_formed(value))
        .take(10)
        .map(|(row_ix, value)| format!("row {} holds '{value}'", row_ix + 2))
        .collect();

    if offenders.is_empty() {
        return Ok(());
    }

    bail!(
        "The '{SYNC_ID_STR}' column of the Transactions tab holds values that Tiller Sync did not \
         write ({}). That column is reserved for Tiller Sync's own row identifiers, which always \
         begin with '{SYNC_ID_PREFIX}'. If the column and its contents are yours, rename it to \
         anything else and run 'tiller sync down' again. If you edited these cells, clearing them \
         lets 'tiller sync down' assign new identifiers, but be aware that a row given a new \
         identifier loses any category, note or tags held only in the local datastore.",
        offenders.join("; ")
    )
}

/// Rejects a repeated identifier, naming the rows that share it.
fn check_unique(existing: &[&str]) -> Res<()> {
    let mut seen: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (row_ix, value) in existing.iter().enumerate() {
        if value.is_empty() {
            continue;
        }
        seen.entry(value).or_default().push(row_ix + 2);
    }

    let duplicates: Vec<String> = seen
        .iter()
        .filter(|(_, rows)| rows.len() > 1)
        .take(10)
        .map(|(value, rows)| {
            let rows: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
            format!("'{value}' on rows {}", rows.join(", "))
        })
        .collect();

    if duplicates.is_empty() {
        return Ok(());
    }

    bail!(
        "The '{SYNC_ID_STR}' column of the Transactions tab repeats a value ({}). Each row needs \
         its own identifier, and Tiller Sync cannot pick which row keeps a repeated one: Tiller \
         adds new rows at the top of the tab, so a copied row can sit above the row it was copied \
         from, and choosing by position would move the original row's local edits onto the copy. \
         Clear the '{SYNC_ID_STR}' cell on the rows that are new and run 'tiller sync down' again.",
        duplicates.join("; ")
    )
}

/// The columns compared before writing, to confirm the sheet has not moved underneath us.
///
/// `Date` and `Transaction ID` are the two columns most likely to distinguish one row from the
/// row above it, and `TillerSyncID` is the column being written. Comparing all three catches a
/// row inserted, deleted or reordered in the seconds between the read and the write, which would
/// otherwise put every identifier on the wrong row.
const WITNESS_COLUMNS: &[&str] = &[SYNC_ID_STR, TRANSACTION_ID_STR, DATE_STR];

/// Confirms that the Transactions tab still looks the way it did when [`assign_sync_ids`] planned its
/// writes.
///
/// Called immediately before writing, with `planned` being the grid the plan was computed from and
/// `current` a fresh download. A sync ID is written by position, so a sheet that has changed shape
/// makes every planned position wrong.
pub(crate) fn check_sync_ids_unmoved(planned: &[Vec<String>], current: &[Vec<String>]) -> Res<()> {
    if planned.len() != current.len() {
        bail!(
            "The Transactions tab changed while 'tiller sync down' was reading it: it had {} rows \
             and now has {}. No identifiers were written. Run 'tiller sync down' again.",
            planned.len().saturating_sub(1),
            current.len().saturating_sub(1)
        );
    }

    let (Some(planned_header), Some(current_header)) = (planned.first(), current.first()) else {
        return Ok(());
    };

    if planned_header != current_header {
        bail!(
            "The header row of the Transactions tab changed while 'tiller sync down' was reading \
             it. No identifiers were written. Run 'tiller sync down' again."
        );
    }

    for name in WITNESS_COLUMNS {
        let Some(col) = planned_header.iter().position(|h| h == name) else {
            continue;
        };
        for row_ix in 1..planned.len() {
            let before = cell(&planned[row_ix], col);
            let after = cell(&current[row_ix], col);
            if before != after {
                bail!(
                    "The Transactions tab changed while 'tiller sync down' was reading it: row {} \
                     held '{before}' in the '{name}' column and now holds '{after}'. No \
                     identifiers were written. Run 'tiller sync down' again.",
                    row_ix + 1
                );
            }
        }
    }

    Ok(())
}

/// Confirms that the write landed: every row of `grid` carries the identifier it was meant to.
///
/// Called on the download taken after the write, which then becomes the data the rest of
/// `sync down` works from.
pub(crate) fn check_sync_ids_written(grid: &[Vec<String>], expected: &SyncIdAssignment) -> Res<()> {
    let Some(header) = grid.first() else {
        bail!("The Transactions tab came back empty after writing sync IDs");
    };

    let Some(col) = header.iter().position(|h| h == SYNC_ID_STR) else {
        bail!(
            "The '{SYNC_ID_STR}' column is missing from the Transactions tab after writing it. \
             Nothing has been saved locally; run 'tiller sync down' again."
        );
    };

    let rows = &grid[1..];
    if rows.len() != expected.ids().len() {
        bail!(
            "The Transactions tab has {} rows after writing sync IDs but {} were expected. \
             Nothing has been saved locally; run 'tiller sync down' again.",
            rows.len(),
            expected.ids().len()
        );
    }

    for (row_ix, (row, want)) in rows.iter().zip(expected.ids()).enumerate() {
        let got = cell(row, col);
        if got != want.as_str() {
            bail!(
                "Row {} of the Transactions tab holds '{got}' in the '{SYNC_ID_STR}' column but \
                 '{want}' was written. Nothing has been saved locally; run 'tiller sync down' \
                 again.",
                row_ix + 2
            );
        }
    }

    let observed: Vec<&str> = rows.iter().map(|row| cell(row, col)).collect();
    check_unique(&observed)?;

    Ok(())
}

/// Reads a cell, treating a row that stops short of `col` as holding an empty value.
fn cell(row: &[String], col: usize) -> &str {
    row.get(col).map(|s| s.trim()).unwrap_or_default()
}

/// Builds the bootstrap seed for each row from its `Transaction ID`.
///
/// Only a value that is non-blank and unique across the whole tab can seed a row. A repeated
/// `Transaction ID` is not an error; it simply means those rows are minted instead.
fn seeds_by_row(header: &[String], rows: &[Vec<String>]) -> BTreeMap<usize, String> {
    let Some(id_col) = header.iter().position(|h| h == TRANSACTION_ID_STR) else {
        return BTreeMap::new();
    };

    let values: Vec<&str> = rows.iter().map(|row| cell(row, id_col)).collect();

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for value in &values {
        if !value.is_empty() {
            *counts.entry(value).or_default() += 1;
        }
    }

    values
        .iter()
        .enumerate()
        .filter(|(_, value)| counts.get(*value).copied() == Some(1))
        .filter_map(|(row_ix, value)| seed_from_transaction_id(value).map(|seed| (row_ix, seed)))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    fn grid(headers: &[&str], rows: &[&[&str]]) -> Vec<Vec<String>> {
        let mut out = vec![headers.iter().map(|s| s.to_string()).collect()];
        out.extend(
            rows.iter()
                .map(|row| row.iter().map(|s| s.to_string()).collect()),
        );
        out
    }

    #[test]
    fn test_minted_ids_are_well_formed() {
        let id = mint_sync_id();
        assert!(id.starts_with(SYNC_ID_PREFIX));
        assert!(is_well_formed(&id));
    }

    #[test]
    fn test_is_well_formed_rejects_foreign_values() {
        assert!(!is_well_formed(""));
        assert!(!is_well_formed("sync-"));
        assert!(!is_well_formed("69112cec0a57f52108456b88"));
        assert!(!is_well_formed("Reconciled"));
        assert!(!is_well_formed("sync-has space"));
    }

    #[test]
    fn test_seed_from_transaction_id() {
        assert_eq!(
            seed_from_transaction_id("69112cec0a57f52108456b88"),
            Some("sync-69112cec0a57f52108456b88".to_string())
        );
        assert_eq!(seed_from_transaction_id(""), None);
        assert_eq!(seed_from_transaction_id("split:[1]"), None);
    }

    /// The bootstrap seeds from `Transaction ID` where it is unique, and mints for the rest.
    #[test]
    fn test_assign_bootstrap_seeds_unique_transaction_ids() {
        let g = grid(
            &["Transaction ID", "Date", "Amount"],
            &[
                &["aaa111", "2025-01-01", "-1.00"],
                &["", "2025-01-02", "-2.00"],
                &["split:[1]", "2025-01-03", "-3.00"],
                &["split:[1]", "2025-01-04", "-4.00"],
            ],
        );

        let assignment = assign_sync_ids(&g, &HashSet::new()).unwrap();

        assert!(assignment.creates_column());
        assert_eq!(assignment.column(), 3);
        assert_eq!(assignment.seeded(), 1);
        assert_eq!(assignment.minted(), 3);
        assert_eq!(assignment.ids()[0], "sync-aaa111");
        assert_eq!(assignment.writes().len(), 4);
        for id in assignment.ids() {
            assert!(is_well_formed(id), "{id} is not well formed");
        }
    }

    /// Rows that already carry an identifier keep it, and only the blank rows are written.
    #[test]
    fn test_assign_mints_only_blank_rows() {
        let g = grid(
            &["Transaction ID", "Date", SYNC_ID_STR],
            &[
                &["aaa111", "2025-01-01", "sync-aaa111"],
                &["bbb222", "2025-01-02", ""],
            ],
        );

        let assignment = assign_sync_ids(&g, &HashSet::new()).unwrap();

        assert!(!assignment.creates_column());
        assert_eq!(assignment.column(), 2);
        assert_eq!(assignment.ids()[0], "sync-aaa111");
        assert_eq!(assignment.writes().len(), 1);
        assert_eq!(assignment.writes()[0].0, 1);
        // Past the bootstrap, a blank row is minted rather than seeded from its Transaction ID.
        assert_eq!(assignment.seeded(), 0);
        assert_eq!(assignment.minted(), 1);
        assert_ne!(assignment.ids()[1], "sync-bbb222");
    }

    /// A row shorter than the header, which is how Sheets returns trailing empty cells.
    #[test]
    fn test_assign_handles_short_rows() {
        let g = grid(
            &["Transaction ID", "Date", SYNC_ID_STR],
            &[&["aaa111", "2025-01-01"]],
        );

        let assignment = assign_sync_ids(&g, &HashSet::new()).unwrap();

        assert_eq!(assignment.writes().len(), 1);
        assert!(is_well_formed(&assignment.ids()[0]));
    }

    /// A column that predates us: the header matches, but nothing in it is ours.
    #[test]
    fn test_assign_rejects_a_pre_existing_column() {
        let g = grid(
            &["Transaction ID", "Date", SYNC_ID_STR],
            &[
                &["aaa111", "2025-01-01", "Reconciled"],
                &["bbb222", "2025-01-02", "Pending"],
            ],
        );

        let err = assign_sync_ids(&g, &HashSet::new())
            .unwrap_err()
            .to_string();

        assert!(err.contains("did not write"), "{err}");
        assert!(err.contains("row 2 holds 'Reconciled'"), "{err}");
        assert!(err.contains("rename it"), "{err}");
    }

    /// A copy-pasted row leaves two rows sharing one identifier.
    #[test]
    fn test_assign_rejects_duplicates() {
        let g = grid(
            &["Transaction ID", "Date", SYNC_ID_STR],
            &[
                &["aaa111", "2025-01-01", "sync-aaa111"],
                &["aaa111", "2025-01-01", "sync-aaa111"],
            ],
        );

        let err = assign_sync_ids(&g, &HashSet::new())
            .unwrap_err()
            .to_string();

        assert!(err.contains("repeats a value"), "{err}");
        assert!(err.contains("'sync-aaa111' on rows 2, 3"), "{err}");
    }

    /// Nothing to do when every row already carries an identifier.
    #[test]
    fn test_assign_is_a_noop_in_the_steady_state() {
        let g = grid(
            &["Transaction ID", SYNC_ID_STR],
            &[&["aaa111", "sync-aaa111"], &["bbb222", "sync-bbb222"]],
        );

        let assignment = assign_sync_ids(&g, &HashSet::new()).unwrap();

        assert!(assignment.is_noop());
        assert_eq!(assignment.writes().len(), 0);
    }

    /// The seed a row gets is the identifier the datastore already holds for it, which is what
    /// makes the upgrade seamless. Finding it among the known identifiers is the expected result,
    /// not a collision.
    #[test]
    fn test_assign_seeds_an_identifier_the_datastore_already_holds() {
        let mut known = HashSet::new();
        known.insert("sync-aaa111".to_string());

        let g = grid(&["Transaction ID", "Date"], &[&["aaa111", "2025-01-01"]]);
        let assignment = assign_sync_ids(&g, &known).unwrap();

        assert_eq!(assignment.ids()[0], "sync-aaa111");
        assert_eq!(assignment.seeded(), 1);
        assert_eq!(assignment.minted(), 0);
    }

    /// A minted identifier avoids everything the datastore holds, including rows that are no
    /// longer in the sheet.
    #[test]
    fn test_assign_mints_around_known_ids() {
        let known: HashSet<String> = (0..64).map(|i| format!("sync-known{i}")).collect();

        let g = grid(
            &["Transaction ID", "Date", SYNC_ID_STR],
            &[&["", "2025-01-01", ""]],
        );
        let assignment = assign_sync_ids(&g, &known).unwrap();

        assert_eq!(assignment.minted(), 1);
        assert!(!known.contains(&assignment.ids()[0]));
    }
}
