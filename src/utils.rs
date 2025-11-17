use crate::Result;
use anyhow::Context;
use serde::de::DeserializeOwned;
use std::path::Path;

/// Write a file.
pub(crate) async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    tokio::fs::write(path, contents)
        .await
        .context(format!("Unable to write to {}", path.to_string_lossy()))
}

/// Read a file to a `String`.
pub async fn read(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read file at {}", path.display()))
}

/// Deserialize a JSON file into type `T`.
pub async fn deserialize<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    let content = read(path).await?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON file at {}", path.display()))
}

/// Basically move a file. Renames `from` -> `to`.
pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
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
