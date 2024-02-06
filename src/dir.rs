use crate::fs;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub(crate) struct Dir {
    root: PathBuf,
}

impl Dir {
    pub(crate) fn new(root: impl AsRef<Path>) -> Result<Self> {
        let home = root.as_ref();
        let myself = Self {
            root: home.to_path_buf(),
        };
        if myself.layout_file().is_file() {
            let contents = fs::read_to_string(myself.layout_file())?;
            bail!(
                "Directory layout {} is unsupported. Is a newer version of fin available?",
                contents
            )
        }
        fs::create_dir_all(myself.config())?;
        fs::create_dir_all(myself.db())?;
        if !myself.layout_file().is_file() {
            fs::write_all(myself.layout_file(), "1".bytes())
                .context("Unable to write directory layout file")?;
        }
        Ok(myself)
    }

    pub(crate) fn root(&self) -> &Path {
        self.root.as_path()
    }

    pub(crate) fn config(&self) -> PathBuf {
        self.root().join("config")
    }

    pub(crate) fn db(&self) -> PathBuf {
        self.root().join("db")
    }

    pub(crate) fn layout_file(&self) -> PathBuf {
        self.root().join(".dir_layout")
    }
}

#[test]
fn create_dir_test() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let home = tempdir.path().join("x");
    let dir = Dir::new(&home).unwrap();
    assert!(dir.root().is_dir());
    assert!(dir.config().is_dir());
    assert!(dir.db().is_dir());
    assert!(dir.layout_file().is_file());
}

#[test]
fn create_dir_exists_test() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let home = tempdir.path().join("y");
    fs::create_dir_all(&home).unwrap();
    let dir = Dir::new(&home).unwrap();
    assert!(dir.root().is_dir());
    assert!(dir.config().is_dir());
    assert!(dir.db().is_dir());
    assert!(dir.layout_file().is_file());
}

#[test]
fn create_dir_bad_layout_version() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let home = tempdir.path().join("z");
    fs::create_dir_all(&home).unwrap();
    fs::write_all(home.join(".dir_layout"), "999999".bytes()).unwrap();
    assert!(Dir::new(&home).is_err());
}
