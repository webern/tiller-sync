//! Comparing two `TillerData` snapshots.
//!
//! Used to answer two questions that look the same but are asked at different moments:
//!
//! - Before `sync down`: has the local datastore been edited since it was last in step with the
//!   sheet? If so, downloading would overwrite those edits.
//! - Before `sync up`: has the sheet been edited since it was last downloaded? If so, uploading
//!   would overwrite those edits.
//!
//! See <https://github.com/webern/tiller-sync/issues/38>.

use crate::model::items::Item;
use crate::model::{AutoCats, Categories, TillerData, Transactions};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

/// How one tab differs between two snapshots.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TabChanges {
    /// Rows present in the newer snapshot but not the older one.
    pub added: usize,
    /// Rows present in both, with at least one differing cell.
    pub modified: usize,
    /// Rows present in the older snapshot but not the newer one.
    pub removed: usize,
}

impl TabChanges {
    pub fn is_empty(&self) -> bool {
        self.added == 0 && self.modified == 0 && self.removed == 0
    }

    fn total(&self) -> usize {
        self.added + self.modified + self.removed
    }
}

/// How two `TillerData` snapshots differ, tab by tab.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Changes {
    /// Differences in the Transactions tab.
    pub transactions: TabChanges,
    /// Differences in the Categories tab.
    pub categories: TabChanges,
    /// Differences in the AutoCat tab.
    pub auto_cats: TabChanges,
}

impl Changes {
    /// Compares `newer` against `older`, describing what `newer` did to `older`.
    pub(crate) fn between(older: &TillerData, newer: &TillerData) -> Self {
        Self {
            transactions: transaction_changes(&older.transactions, &newer.transactions),
            categories: category_changes(&older.categories, &newer.categories),
            auto_cats: auto_cat_changes(&older.auto_cats, &newer.auto_cats),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty() && self.categories.is_empty() && self.auto_cats.is_empty()
    }
}

impl Display for Changes {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return write!(f, "no changes");
        }

        let mut parts = Vec::new();
        for (name, tab) in [
            ("transaction", self.transactions),
            ("category", self.categories),
            ("autocat rule", self.auto_cats),
        ] {
            if tab.is_empty() {
                continue;
            }
            parts.push(format!(
                "{} {name}{} ({} added, {} modified, {} deleted)",
                tab.total(),
                if tab.total() == 1 { "" } else { "s" },
                tab.added,
                tab.modified,
                tab.removed
            ));
        }
        write!(f, "{}", parts.join(", "))
    }
}

/// Comparison uses each row's sheet representation rather than its in-memory fields.
///
/// A row's sheet representation is the canonical form of the data: it is exactly what a `sync up`
/// would write and exactly what a `sync down` parsed. Comparing typed fields instead would report
/// differences for values that round-trip through the database with a different but equivalent
/// representation, which would make the check cry wolf and get ignored.
fn rows<I>(items: &[I], headers: &[String]) -> Vec<Vec<String>>
where
    I: Item,
{
    items.iter().map(|item| item.to_row(headers)).collect()
}

/// The headers to compare a tab's rows across: the sheet's own columns.
///
/// Deliberately not "every field the model has". A field with no column in the sheet cannot be
/// uploaded, so reporting an edit to it would produce a difference that no `sync up` could ever
/// clear, and `sync down` would be blocked forever. The comparison measures what a sync can
/// actually carry.
///
/// The unnamed column A that some sheets have is dropped for the opposite reason: it has no
/// database column, so it reads back empty and would make every row look modified.
fn headers_of(mapping: &crate::model::Mapping) -> Vec<String> {
    mapping
        .headers()
        .iter()
        .map(|h| h.as_ref().to_string())
        .filter(|h| !h.is_empty())
        .collect()
}

/// Compares two keyed tabs, where `key` extracts a stable identity for a row.
fn keyed_changes(
    older: Vec<(String, Vec<String>)>,
    newer: Vec<(String, Vec<String>)>,
) -> TabChanges {
    let older: BTreeMap<String, Vec<String>> = older.into_iter().collect();
    let newer: BTreeMap<String, Vec<String>> = newer.into_iter().collect();

    let mut changes = TabChanges::default();
    for (key, new_row) in &newer {
        match older.get(key) {
            None => changes.added += 1,
            Some(old_row) if old_row != new_row => changes.modified += 1,
            Some(_) => {}
        }
    }
    changes.removed = older.keys().filter(|key| !newer.contains_key(*key)).count();
    changes
}

fn transaction_changes(older: &Transactions, newer: &Transactions) -> TabChanges {
    let headers = headers_of(newer.mapping());
    let keyed = |items: &Transactions| -> Vec<(String, Vec<String>)> {
        items
            .data()
            .iter()
            .map(|t| t.transaction_id.clone())
            .zip(rows(items.data(), &headers))
            .collect()
    };
    keyed_changes(keyed(older), keyed(newer))
}

fn category_changes(older: &Categories, newer: &Categories) -> TabChanges {
    let headers = headers_of(newer.mapping());
    let keyed = |items: &Categories| -> Vec<(String, Vec<String>)> {
        items
            .data()
            .iter()
            .map(|c| c.category.clone())
            .zip(rows(items.data(), &headers))
            .collect()
    };
    keyed_changes(keyed(older), keyed(newer))
}

/// AutoCat rules have no stable identity: their primary key is a synthetic auto-increment that is
/// reassigned on every `sync down`, because the whole tab is replaced. Rules are therefore compared
/// as a multiset of their sheet rows, which reports a reordering as no change at all. That is the
/// right answer here: the question is whether rules were gained or lost, and the sheet's own row
/// order is not something the user edits meaningfully.
fn auto_cat_changes(older: &AutoCats, newer: &AutoCats) -> TabChanges {
    let headers = headers_of(newer.mapping());
    let counted = |items: &AutoCats| -> BTreeMap<Vec<String>, usize> {
        let mut counts = BTreeMap::new();
        for row in rows(items.data(), &headers) {
            *counts.entry(row).or_default() += 1;
        }
        counts
    };
    let older = counted(older);
    let newer = counted(newer);

    let mut changes = TabChanges::default();
    for (row, new_count) in &newer {
        let old_count = older.get(row).copied().unwrap_or(0);
        changes.added += new_count.saturating_sub(old_count);
    }
    for (row, old_count) in &older {
        let new_count = newer.get(row).copied().unwrap_or(0);
        changes.removed += old_count.saturating_sub(new_count);
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AutoCats, Categories, Transactions};

    fn transactions(rows: &[(&str, &str, &str)]) -> Transactions {
        let mut sheet = vec![vec![
            "Transaction ID".to_string(),
            "Date".to_string(),
            "Amount".to_string(),
            "Category".to_string(),
        ]];
        for (id, amount, category) in rows {
            sheet.push(vec![
                id.to_string(),
                "2025-01-01".to_string(),
                amount.to_string(),
                category.to_string(),
            ]);
        }
        Transactions::parse(sheet, Vec::<Vec<String>>::new()).unwrap()
    }

    fn categories(names: &[&str]) -> Categories {
        let mut sheet = vec![vec!["Category".to_string(), "Group".to_string()]];
        for name in names {
            sheet.push(vec![name.to_string(), "Everyday".to_string()]);
        }
        Categories::parse(sheet, Vec::<Vec<String>>::new()).unwrap()
    }

    fn auto_cats(contains: &[&str]) -> AutoCats {
        let mut sheet = vec![vec![
            "Category".to_string(),
            "Description Contains".to_string(),
        ]];
        for text in contains {
            sheet.push(vec!["Groceries".to_string(), text.to_string()]);
        }
        AutoCats::parse(sheet, Vec::<Vec<String>>::new()).unwrap()
    }

    fn data(
        txns: &[(&str, &str, &str)],
        cats: &[&str],
        rules: &[&str],
    ) -> crate::model::TillerData {
        crate::model::TillerData {
            transactions: transactions(txns),
            categories: categories(cats),
            auto_cats: auto_cats(rules),
        }
    }

    #[test]
    fn test_identical_data_has_no_changes() {
        let a = data(&[("t1", "-1.00", "Food")], &["Food"], &["starbucks"]);
        let b = data(&[("t1", "-1.00", "Food")], &["Food"], &["starbucks"]);
        let changes = Changes::between(&a, &b);
        assert!(changes.is_empty(), "got: {changes}");
        assert_eq!(changes.to_string(), "no changes");
    }

    /// Recategorizing a transaction is the edit the reporter nearly lost.
    #[test]
    fn test_recategorizing_counts_as_a_modification() {
        let before = data(&[("t1", "-1.00", "")], &["Food"], &[]);
        let after = data(&[("t1", "-1.00", "Food")], &["Food"], &[]);
        let changes = Changes::between(&before, &after);

        assert_eq!(changes.transactions.modified, 1);
        assert_eq!(changes.transactions.added, 0);
        assert_eq!(changes.transactions.removed, 0);
        assert!(changes.categories.is_empty());
    }

    #[test]
    fn test_added_and_removed_transactions() {
        let before = data(&[("t1", "-1.00", "Food")], &["Food"], &[]);
        let after = data(&[("t2", "-2.00", "Food")], &["Food"], &[]);
        let changes = Changes::between(&before, &after);

        assert_eq!(changes.transactions.added, 1);
        assert_eq!(changes.transactions.removed, 1);
        assert_eq!(changes.transactions.modified, 0);
    }

    /// AutoCat rules added locally are the other thing the reporter nearly lost, and they have no
    /// stable key to match on.
    #[test]
    fn test_added_autocat_rules() {
        let before = data(&[], &["Groceries"], &["starbucks"]);
        let after = data(&[], &["Groceries"], &["starbucks", "peets", "blue bottle"]);
        let changes = Changes::between(&before, &after);

        assert_eq!(changes.auto_cats.added, 2);
        assert_eq!(changes.auto_cats.removed, 0);
    }

    #[test]
    fn test_removed_autocat_rules() {
        let before = data(&[], &["Groceries"], &["starbucks", "peets"]);
        let after = data(&[], &["Groceries"], &["starbucks"]);
        let changes = Changes::between(&before, &after);

        assert_eq!(changes.auto_cats.removed, 1);
        assert_eq!(changes.auto_cats.added, 0);
    }

    /// Reordering the AutoCat tab is not a change worth blocking a sync over.
    #[test]
    fn test_reordered_autocat_rules_are_not_changes() {
        let before = data(&[], &["Groceries"], &["starbucks", "peets"]);
        let after = data(&[], &["Groceries"], &["peets", "starbucks"]);
        let changes = Changes::between(&before, &after);

        assert!(changes.auto_cats.is_empty(), "got: {changes}");
    }

    #[test]
    fn test_category_changes() {
        let before = data(&[], &["Food", "Gas"], &[]);
        let after = data(&[], &["Food", "Fuel"], &[]);
        let changes = Changes::between(&before, &after);

        assert_eq!(changes.categories.added, 1);
        assert_eq!(changes.categories.removed, 1);
    }

    #[test]
    fn test_display_summarizes_every_tab() {
        let before = data(&[("t1", "-1.00", "")], &["Food"], &[]);
        let after = data(
            &[("t1", "-1.00", "Food"), ("t2", "-2.00", "Food")],
            &["Food", "Gas"],
            &["shell"],
        );
        let summary = Changes::between(&before, &after).to_string();

        assert!(summary.contains("2 transactions"), "got: {summary}");
        assert!(summary.contains("1 category"), "got: {summary}");
        assert!(summary.contains("1 autocat rule"), "got: {summary}");
    }
}
