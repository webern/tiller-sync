use crate::Result;
use anyhow::Context;
use std::path::{Path, PathBuf};
use tokio::fs;

/// The `Home` object represents the file paths of the `$TILLER_HOME` directory and those paths
/// which are not configurable within `$TILLER_HOME` such as `$TILLER_HOME/config.json`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Home {
    root: PathBuf,
    backups: PathBuf,
    secrets: PathBuf,
    config: PathBuf,
    db: PathBuf,
}

impl Home {
    /// This will create the `tiller_home` directory, if it does not exist, and canonicalize itself.
    pub async fn new(tiller_home: impl Into<PathBuf>) -> Result<Self> {
        let maybe_relative = tiller_home.into();
        make_dir(&maybe_relative)
            .await
            .context("Unable to create tiller home directory")?;
        let root = fs::canonicalize(&maybe_relative).await.with_context(|| {
            format!(
                "Unable to canonicalize the path {}",
                maybe_relative.to_string_lossy()
            )
        })?;
        let home = Self {
            root: root.clone(),
            backups: root.join(".backups"),
            secrets: root.join(".secrets"),
            config: root.join("config.json"),
            db: root.join("tiller.sqlite"),
        };
        make_dir(&home.backups).await?;
        make_dir(&home.secrets).await?;
        Ok(home)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config(&self) -> &Path {
        &self.config
    }

    pub fn db(&self) -> &Path {
        &self.db
    }

    pub(crate) fn _backups(&self) -> &Path {
        &self.backups
    }

    pub(crate) fn _secrets(&self) -> &Path {
        &self.secrets
    }
}

async fn make_dir(p: &Path) -> Result<()> {
    fs::create_dir_all(p)
        .await
        .with_context(|| format!("Unable to create directory at {}", p.to_string_lossy()))
}

#[tokio::test]
async fn test_home() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let home_dir = dir.path().to_owned();
    let home = Home::new(home_dir).await.unwrap();
    assert!(fs::read_dir(home._backups()).await.is_ok());
    assert!(fs::read_dir(home._secrets()).await.is_ok());
}
