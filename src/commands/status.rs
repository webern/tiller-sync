//! The `status` command: what has changed locally, and what has changed in the sheet.
//!
//! This exists so that "have my local edits been uploaded?" and "has the sheet changed since I last
//! downloaded?" can be answered without running a sync. Answering the second question used to
//! require a `sync down`, which is exactly the operation that discards unsynced local edits.
//!
//! See <https://github.com/webern/tiller-sync/issues/38>.

use crate::api::{sheet, tiller, Mode, Tiller};
use crate::backup::SYNC_DOWN;
use crate::commands::sync::local_changes;
use crate::commands::Out;
use crate::error::{ErrorType, IntoResult};
use crate::model::Changes;
use crate::{Config, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// Where the local datastore and the Google Sheet stand relative to the last sync.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SyncStatus {
    /// Whether a sync has ever run. Everything below is meaningless when this is false.
    pub synced_before: bool,
    /// Edits made to the local datastore since the last sync. `sync down` would discard these.
    pub local: Changes,
    /// Edits made to the Google Sheet since the last sync. `sync up` would overwrite these.
    ///
    /// Absent when the sheet was not read, which happens when `check_remote` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<Changes>,
}

impl Display for SyncStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match serde_json::to_string_pretty(self) {
            Ok(s) => write!(f, "{s}"),
            Err(_) => write!(f, "{self:?}"),
        }
    }
}

/// Reports what has changed locally and, optionally, in the Google Sheet.
///
/// Reading the sheet costs an API call and requires valid authentication, so `check_remote` makes
/// it opt-out: the local half of the answer is always available offline.
pub async fn status(config: Config, mode: Mode, check_remote: bool) -> Result<Out<SyncStatus>> {
    let Some(local) = local_changes(&config).await? else {
        return Ok(Out::new(
            "No sync has run yet, so there is nothing to compare against. Run 'tiller sync down' \
             to download the sheet.",
            SyncStatus {
                synced_before: false,
                local: Changes::default(),
                remote: None,
            },
        ));
    };

    let remote = if check_remote {
        Some(remote_changes(&config, mode).await?)
    } else {
        None
    };

    let message = describe(&local, remote.as_ref());
    Ok(Out::new(
        message,
        SyncStatus {
            synced_before: true,
            local,
            remote,
        },
    ))
}

/// Compares the Google Sheet against the last known state of it.
async fn remote_changes(config: &Config, mode: Mode) -> Result<Changes> {
    let last_synced = config
        .backup()
        .load_latest_json(SYNC_DOWN)
        .await
        .pub_result(ErrorType::Internal)?
        .ok_or_else(|| anyhow::anyhow!("No sync-down backup found"))
        .pub_result(ErrorType::Internal)?;

    let sheet_client = sheet(config.clone(), mode).await?;
    let mut tiller_client = tiller(sheet_client).await.pub_result(ErrorType::Internal)?;
    let current = tiller_client.get_data().await.pub_result(ErrorType::Sync)?;

    Ok(Changes::between(&last_synced, &current))
}

/// Turns the comparison into a sentence that says what to do next.
fn describe(local: &Changes, remote: Option<&Changes>) -> String {
    let local_dirty = !local.is_empty();
    let remote_dirty = remote.is_some_and(|r| !r.is_empty());

    match (local_dirty, remote_dirty) {
        (false, false) if remote.is_some() => {
            "Local datastore and sheet are both in step with the last sync.".to_string()
        }
        (false, false) => "No local changes since the last sync.".to_string(),
        (true, false) => format!(
            "Local changes not yet in the sheet: {local}. Run 'tiller sync up' to upload them."
        ),
        (false, true) => format!(
            "The sheet has changed since the last sync: {}. Run 'tiller sync down' to pull them in.",
            remote.unwrap_or(&Changes::default())
        ),
        (true, true) => format!(
            "Both sides have changed. Local: {local}. Sheet: {}. Neither sync direction can \
             preserve both, so reconcile them before syncing, or choose a side with --force.",
            remote.unwrap_or(&Changes::default())
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::DeleteTransactionsArgs;
    use crate::commands::{sync_down, sync_up, FormulasMode};
    use crate::model::TransactionUpdates;
    use crate::test::TestEnv;

    #[tokio::test]
    async fn test_status_before_any_sync() {
        let env = TestEnv::new().await;
        let out = status(env.config(), Mode::Testing, false).await.unwrap();

        assert!(!out.structure().unwrap().synced_before);
        assert!(out.message().contains("No sync has run yet"));
    }

    #[tokio::test]
    async fn test_status_is_clean_right_after_sync_down() {
        let env = TestEnv::new().await;
        let config = env.config();
        sync_down(config.clone(), Mode::Testing, false)
            .await
            .unwrap();

        let out = status(config, Mode::Testing, true).await.unwrap();
        let structure = out.structure().unwrap();

        assert!(structure.local.is_empty(), "local: {}", structure.local);
        assert!(
            structure.remote.unwrap().is_empty(),
            "the sheet has not been touched"
        );
    }

    #[tokio::test]
    async fn test_status_reports_local_edits() {
        let env = TestEnv::new().await;
        let config = env.config();
        sync_down(config.clone(), Mode::Testing, false)
            .await
            .unwrap();

        let data = config.db().get_tiller_data().await.unwrap();
        let id = data.transactions.data()[0].transaction_id.clone();
        let updates = TransactionUpdates {
            category: Some("Restaurants".to_string()),
            ..Default::default()
        };
        let args = crate::args::UpdateTransactionsArgs::new(vec![id], updates).unwrap();
        crate::commands::update_transactions(config.clone(), args)
            .await
            .unwrap();

        let out = status(config, Mode::Testing, false).await.unwrap();
        let structure = out.structure().unwrap();

        assert_eq!(structure.local.transactions.modified, 1);
        assert!(
            out.message().contains("sync up"),
            "the message should say what to do, got: {}",
            out.message()
        );
    }

    /// The remote half is the point of the command: checking for sheet changes used to require a
    /// `sync down`, which is the operation that discards local edits.
    #[tokio::test]
    async fn test_status_reports_remote_edits_without_downloading() {
        let env = TestEnv::new().await;
        let config = env.config();
        sync_down(config.clone(), Mode::Testing, false)
            .await
            .unwrap();

        // Edit the sheet behind the tool's back.
        let mut state = env.get_state();
        state.data.get_mut("Transactions").unwrap()[1][2].push_str(" (edited)");
        env.set_state(state);

        let out = status(config.clone(), Mode::Testing, true).await.unwrap();
        let structure = out.structure().unwrap();

        assert_eq!(structure.remote.unwrap().transactions.modified, 1);
        assert!(structure.local.is_empty(), "the datastore was not touched");

        // And the datastore was genuinely left alone.
        let after = config.db().get_tiller_data().await.unwrap();
        assert!(!after.transactions.data()[0]
            .description
            .contains("(edited)"));
    }

    /// After uploading, both sides agree again. Without a refreshed baseline, `status` and
    /// `sync down` would keep reporting the just-uploaded edits as unsynced forever.
    #[tokio::test]
    async fn test_status_is_clean_after_sync_up() {
        let env = TestEnv::new().await;
        let config = env.config();
        sync_down(config.clone(), Mode::Testing, false)
            .await
            .unwrap();

        let data = config.db().get_tiller_data().await.unwrap();
        let id = data.transactions.data()[0].transaction_id.clone();
        let args = DeleteTransactionsArgs::new(vec![id]).unwrap();
        crate::commands::delete_transactions(config.clone(), args)
            .await
            .unwrap();

        sync_up(config.clone(), Mode::Testing, false, FormulasMode::Ignore)
            .await
            .unwrap();

        let out = status(config, Mode::Testing, true).await.unwrap();
        let structure = out.structure().unwrap();

        assert!(
            structure.local.is_empty(),
            "after uploading, the local datastore is in step: {}",
            structure.local
        );
        assert!(
            structure.remote.unwrap().is_empty(),
            "after uploading, the sheet is in step"
        );
    }
}
