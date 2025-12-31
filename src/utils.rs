use crate::error::Res;
use anyhow::Context;
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

/// Basically move a file. Renames `from` -> `to`.
pub(crate) async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Res<()> {
    tokio::fs::rename(from.as_ref(), to.as_ref())
        .await
        .with_context(|| {
            format!(
                "Unable to copy file from '{}' to '{}'",
                from.as_ref().to_string_lossy(),
                to.as_ref().to_string_lossy()
            )
        })
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
