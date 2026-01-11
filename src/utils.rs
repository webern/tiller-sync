use crate::error::Res;
use anyhow::{anyhow, Context};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use tokio::fs::ReadDir;

/// Write a file.
pub(crate) async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Res<()> {
    let path = path.as_ref();
    tokio::fs::write(path, contents)
        .await
        .context(format!("Unable to write to {}", path.to_string_lossy()))
}

/// Read a file to a `String`.
pub(crate) async fn read(path: &Path) -> Res<String> {
    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read file at {}", path.display()))
}

/// Deserialize a JSON file into type `T`.
pub(crate) async fn deserialize<T>(path: &Path) -> Res<T>
where
    T: DeserializeOwned,
{
    let content = read(path).await?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON file at {}", path.display()))
}

pub(crate) async fn canonicalize(path: impl AsRef<Path>) -> Res<PathBuf> {
    tokio::fs::canonicalize(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to canonicalize path '{}'",
                path.as_ref().to_string_lossy()
            )
        })
}

pub(crate) async fn make_dir(path: impl AsRef<Path>) -> Res<()> {
    tokio::fs::create_dir_all(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to create directory at {}",
                path.as_ref().to_string_lossy()
            )
        })
}

pub(crate) async fn read_dir(path: impl AsRef<Path>) -> Res<ReadDir> {
    tokio::fs::read_dir(path.as_ref()).await.with_context(|| {
        format!(
            "Unable to run read_dir on {}",
            path.as_ref().to_string_lossy()
        )
    })
}

/// Copy a file from `from` to `to`.
pub(crate) async fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Res<u64> {
    tokio::fs::copy(from.as_ref(), to.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to copy file from '{}' to '{}'",
                from.as_ref().to_string_lossy(),
                to.as_ref().to_string_lossy()
            )
        })
}

/// Remove a file.
pub(crate) async fn remove(path: impl AsRef<Path>) -> Res<()> {
    tokio::fs::remove_file(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to remove file at '{}'",
                path.as_ref().to_string_lossy()
            )
        })
}

/// Parses update strings in "FIELD=VALUE" format into `("FIELD", "VALUE")`.
pub(crate) fn parse_key_val(key_val: &str) -> Res<(String, String)> {
    key_val
        .split_once('=')
        .map(|x| (x.0.to_string(), x.1.to_string()))
        .ok_or_else(|| anyhow!("Invalid format '{}', expected FIELD=VALUE", key_val))
}

/// Parses an amount string into an `Amount`.
pub(crate) fn parse_amount(s: &str) -> Res<crate::model::Amount> {
    s.parse()
        .with_context(|| format!("Invalid amount format: '{}'", s))
}

/// Generates a unique transaction ID for locally-created transactions.
///
/// The ID format is `user-` followed by a truncated UUIDv4 (dashes removed, truncated to 19
/// characters), resulting in IDs like `user-f47e8c2a9b3d4f1ea80`.
///
/// This distinguishes locally-created transactions from those created by Tiller, which use
/// 24-character hex IDs without a prefix.
pub fn generate_transaction_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    let hex = uuid.as_simple().to_string(); // 32 hex chars, no dashes
    format!("user-{}", &hex[..19])
}
