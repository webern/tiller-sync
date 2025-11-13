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

pub async fn read(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Failed to read file at {}", path.display()))
}

pub async fn deserialize<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned,
{
    let content = read(path).await?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON file at {}", path.display()))
}
