use crate::Result;
use anyhow::Context;
use std::path::Path;

/// Write a file.
pub(crate) async fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    tokio::fs::write(path, contents)
        .await
        .context(format!("Unable to write to {}", path.to_string_lossy()))
}
