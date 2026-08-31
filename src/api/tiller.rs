//! Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.

use crate::api::{
    Sheet, SheetRange, SyncIds, Tiller, WriteCounts, AUTO_CAT, CATEGORIES, TRANSACTIONS,
};
use crate::error::Res;
use crate::model::{
    assign_sync_ids, check_sync_ids_unmoved, check_sync_ids_written, AutoCats, Categories,
    SyncIdAssignment, TillerData, Transactions, SYNC_ID_STR,
};
use std::collections::HashSet;
use tracing::{debug, info};

/// Implements the `Tiller` trait for interacting with Google sheet data from a tiller sheet.
pub(super) struct TillerImpl {
    sheet: Box<dyn Sheet + Send>,
}

impl TillerImpl {
    /// Create a new `TillerImpl` object that will use a dynamically-dispatched `sheet` to get and
    /// send its data.
    pub(super) async fn new(sheet: Box<dyn Sheet + Send>) -> Res<Self> {
        Ok(Self { sheet })
    }
}

#[async_trait::async_trait]
impl Tiller for TillerImpl {
    async fn get_data(&mut self, sync_ids: SyncIds<'_>) -> Res<TillerData> {
        // Fetch data from all three tabs
        let transactions = fetch_transactions(self.sheet.as_mut(), sync_ids).await?;
        let categories = fetch_categories(self.sheet.as_mut()).await?;
        let auto_cats = fetch_auto_cats(self.sheet.as_mut()).await?;

        Ok(TillerData {
            transactions,
            categories,
            auto_cats,
        })
    }

    async fn copy_spreadsheet(&mut self, new_name: &str) -> Res<String> {
        self.sheet.copy_spreadsheet(new_name).await
    }

    async fn clear_and_write_data(
        &mut self,
        data: &TillerData,
        preserve_formulas: bool,
    ) -> Res<usize> {
        // Clear each tab entirely (headers and data)
        let clear_ranges = [
            &format!("{TRANSACTIONS}!A1:ZZ"),
            &format!("{CATEGORIES}!A1:ZZ"),
            &format!("{AUTO_CAT}!A1:ZZ"),
        ];
        self.sheet
            .clear_ranges(&clear_ranges.map(|s| s.as_str()))
            .await?;

        // Build write data for all three sheets (headers + data in one operation each). Formulas
        // are overlaid onto the same grid rather than written separately, so a single write carries
        // both the values and the formulas and there is no window in which the sheet holds one but
        // not the other.
        let mut write_data = Vec::new();
        let mut formulas_written = 0;

        let tabs = [
            (
                TRANSACTIONS,
                data.transactions.to_rows_for_write(preserve_formulas),
            ),
            (
                CATEGORIES,
                data.categories.to_rows_for_write(preserve_formulas),
            ),
            (
                AUTO_CAT,
                data.auto_cats.to_rows_for_write(preserve_formulas),
            ),
        ];

        for (tab, rows) in tabs {
            let (values, count) = rows?;
            if preserve_formulas {
                debug!("Writing {count} formulas to the {tab} tab");
            }
            formulas_written += count;
            write_data.push(SheetRange {
                range: format!("{tab}!A1:ZZ"),
                values,
            });
        }

        self.sheet.write_ranges(&write_data).await?;

        Ok(formulas_written)
    }

    async fn verify_write(&mut self, expected: &TillerData) -> Res<WriteCounts> {
        use anyhow::bail;

        // Re-fetch data from sheets to verify row counts. Verification must not assign sync IDs:
        // `sync up` has just written the whole tab from the datastore, so every row already has
        // one, and a write here would be a write the user did not ask for.
        let actual = self.get_data(SyncIds::Read).await?;

        let expected_txn = expected.transactions.data().len();
        let expected_cat = expected.categories.data().len();
        let expected_ac = expected.auto_cats.data().len();

        let counts = WriteCounts {
            transactions: actual.transactions.data().len(),
            categories: actual.categories.data().len(),
            auto_cats: actual.auto_cats.data().len(),
            formulas: actual.transactions.formulas().len()
                + actual.categories.formulas().len()
                + actual.auto_cats.formulas().len(),
        };

        if counts.transactions != expected_txn {
            bail!(
                "Verification failed: expected {} transactions, found {}",
                expected_txn,
                counts.transactions
            );
        }

        if counts.categories != expected_cat {
            bail!(
                "Verification failed: expected {} categories, found {}",
                expected_cat,
                counts.categories
            );
        }

        if counts.auto_cats != expected_ac {
            bail!(
                "Verification failed: expected {} autocat rules, found {}",
                expected_ac,
                counts.auto_cats
            );
        }

        Ok(counts)
    }
}

/// Fetches transaction data from the Transactions tab, assigning sync IDs when asked to.
async fn fetch_transactions(
    client: &mut (dyn Sheet + Send),
    sync_ids: SyncIds<'_>,
) -> Res<Transactions> {
    let mut values = client.get(TRANSACTIONS).await?;
    if let SyncIds::Assign(known) = sync_ids {
        values = assign_and_write(client, values, known).await?;
    }
    let formulas = client.get_formulas(TRANSACTIONS).await?;
    Transactions::parse(values, formulas)
}

/// The sync ID assignment phase of `sync down`.
///
/// Gives every row of `values` that lacks a sync ID one, writes the new identifiers into the
/// Transactions tab, and returns the tab as read back afterwards. That read-back is what the rest
/// of `sync down` works from, which is what makes the JSON snapshot an exact record of the sheet.
///
/// The write happens and is verified before the caller commits anything to SQLite. If it fails,
/// nothing has been saved locally and the next `sync down` is a clean retry. If the process dies
/// after the write but before the commit, the identifiers are already in the sheet and the next
/// `sync down` simply reads them.
async fn assign_and_write(
    client: &mut (dyn Sheet + Send),
    values: Vec<Vec<String>>,
    known: &HashSet<String>,
) -> Res<Vec<Vec<String>>> {
    let assignment = assign_sync_ids(&values, known)?;

    // The steady state: every row already carries an identifier, so `sync down` writes nothing.
    if assignment.is_noop() {
        return Ok(values);
    }

    if assignment.creates_column() {
        // Creating the column is the one assignment that touches the shape of the sheet, so it
        // gets the same Drive copy that `sync up` takes. Routine assignment of a few new rows
        // writes only cells that were empty and takes no copy.
        let backup_name = format!(
            "tiller-backup-before-sync-ids-{}",
            chrono::Local::now().format("%Y-%m-%d-%H%M%S")
        );
        let backup_id = client.copy_spreadsheet(&backup_name).await?;
        info!("Created Google Sheet backup '{backup_name}' (ID: {backup_id})");
    }

    // A sync ID is written by position, so a sheet that changed shape since the read would put
    // every identifier on the wrong row. Confirm it did not before writing anything.
    let current = client.get(TRANSACTIONS).await?;
    check_sync_ids_unmoved(&values, &current)?;

    let ranges = assignment_ranges(&assignment);
    let sheet_ranges: Vec<SheetRange> = ranges
        .into_iter()
        .map(|(range, values)| SheetRange { range, values })
        .collect();
    client.write_ranges(&sheet_ranges).await?;

    let after = client.get(TRANSACTIONS).await?;
    check_sync_ids_written(&after, &assignment)?;

    if assignment.creates_column() {
        info!(
            "Created the '{SYNC_ID_STR}' column in the Transactions tab and assigned {} sync IDs \
             ({} seeded from an existing Transaction ID, {} newly minted)",
            assignment.writes().len(),
            assignment.seeded(),
            assignment.minted()
        );
    } else {
        info!("Assigned {} new sync IDs", assignment.writes().len());
    }

    Ok(after)
}

/// Builds the cell ranges that carry an assignment into the sheet.
///
/// Only cells whose value changed are written: the header cell when the column was just created,
/// and the rows that received an identifier, grouped into contiguous runs. A cell that already
/// held the right value is never rewritten.
fn assignment_ranges(assignment: &SyncIdAssignment) -> Vec<(String, Vec<Vec<String>>)> {
    let col = column_letters(assignment.column());
    let mut ranges = Vec::new();

    if assignment.creates_column() {
        ranges.push((
            format!("{TRANSACTIONS}!{col}1"),
            vec![vec![SYNC_ID_STR.to_string()]],
        ));
    }

    // Sheet rows are 1-based and the header occupies row 1, so data row 0 is sheet row 2.
    for run in contiguous_runs(assignment.writes()) {
        let first = run[0].0 + 2;
        let last = run[run.len() - 1].0 + 2;
        let values: Vec<Vec<String>> = run.iter().map(|(_, id)| vec![id.clone()]).collect();
        ranges.push((format!("{TRANSACTIONS}!{col}{first}:{col}{last}"), values));
    }

    ranges
}

/// Groups writes that sit on consecutive rows so they travel as one range.
fn contiguous_runs(writes: &[(usize, String)]) -> Vec<Vec<(usize, String)>> {
    let mut runs: Vec<Vec<(usize, String)>> = Vec::new();
    for (row, id) in writes {
        match runs.last_mut() {
            Some(run) if run[run.len() - 1].0 + 1 == *row => run.push((*row, id.clone())),
            _ => runs.push(vec![(*row, id.clone())]),
        }
    }
    runs
}

/// Converts a 0-based column index into its spreadsheet letters: 0 is `A`, 26 is `AA`.
fn column_letters(mut index: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push(b'A' + (index % 26) as u8);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    letters.reverse();
    String::from_utf8_lossy(&letters).into_owned()
}

/// Fetches category data from the Categories tab
async fn fetch_categories(client: &mut (dyn Sheet + Send)) -> Res<Categories> {
    let values = client.get(CATEGORIES).await?;
    let formulas = client.get_formulas(CATEGORIES).await?;
    Categories::parse(values, formulas)
}

/// Fetches AutoCat data from the AutoCat tab
async fn fetch_auto_cats(client: &mut (dyn Sheet + Send)) -> Res<AutoCats> {
    let values = client.get(AUTO_CAT).await?;
    let formulas = client.get_formulas(AUTO_CAT).await?;
    AutoCats::parse(values, formulas)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::model::assign_sync_ids;

    #[test]
    fn test_column_letters() {
        assert_eq!(column_letters(0), "A");
        assert_eq!(column_letters(25), "Z");
        assert_eq!(column_letters(26), "AA");
        assert_eq!(column_letters(27), "AB");
        assert_eq!(column_letters(51), "AZ");
        assert_eq!(column_letters(52), "BA");
        assert_eq!(column_letters(701), "ZZ");
    }

    fn grid(rows: &[&[&str]]) -> Vec<Vec<String>> {
        rows.iter()
            .map(|row| row.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    /// The bootstrap writes the header cell and one range covering every row.
    #[test]
    fn test_assignment_ranges_for_a_new_column() {
        let g = grid(&[
            &["Transaction ID", "Date"],
            &["aaa111", "2025-01-01"],
            &["bbb222", "2025-01-02"],
        ]);
        let assignment = assign_sync_ids(&g, &HashSet::new()).unwrap();
        let ranges = assignment_ranges(&assignment);

        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].0, "Transactions!C1");
        assert_eq!(ranges[0].1, vec![vec![SYNC_ID_STR.to_string()]]);
        assert_eq!(ranges[1].0, "Transactions!C2:C3");
        assert_eq!(ranges[1].1.len(), 2);
    }

    /// Rows that already carry an identifier are skipped, and the gaps between the rows that do
    /// not split the write into separate ranges rather than rewriting cells in between.
    #[test]
    fn test_assignment_ranges_skip_rows_that_already_have_an_id() {
        let g = grid(&[
            &["Transaction ID", SYNC_ID_STR],
            &["aaa111", ""],
            &["bbb222", "sync-bbb222"],
            &["ccc333", ""],
            &["ddd444", ""],
        ]);
        let assignment = assign_sync_ids(&g, &HashSet::new()).unwrap();
        let ranges = assignment_ranges(&assignment);

        // No header cell, because the column already exists.
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].0, "Transactions!B2:B2");
        assert_eq!(ranges[1].0, "Transactions!B4:B5");
        assert_eq!(ranges[1].1.len(), 2);
    }
}
