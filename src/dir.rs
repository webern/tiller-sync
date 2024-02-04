use crate::fs;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub(crate) struct Dir {
    home: PathBuf,
}

impl Dir {
    pub(crate) fn new(home: impl AsRef<Path>) -> Result<Self> {
        let home = home.as_ref();
        let myself = Self {
            home: home.to_path_buf(),
        };
        if myself.dir_layout().is_file() {
            let contents = fs::read_to_string(myself.dir_layout())?;
            bail!(
                "Directory layour {} is unsupported. Is a newer version of fin available?",
                contents
            )
        }
        fs::create_dir_all(myself.config())?;
        fs::create_dir_all(myself.db())?;
        fs::create_dir_all(myself.backups())?;
        if !myself.dir_layout().is_file() {
            fs::write_all(myself.dir_layout(), "1".bytes())
                .context("Unable to write directory layout file")?;
        }
        Ok(myself)
    }

    pub(crate) fn home(&self) -> &Path {
        self.home.as_path()
    }

    pub(crate) fn config(&self) -> PathBuf {
        self.home().join("config")
    }

    pub(crate) fn db(&self) -> PathBuf {
        self.home().join("db")
    }

    pub(crate) fn backups(&self) -> PathBuf {
        self.home().join("backups")
    }

    pub(crate) fn dir_layout(&self) -> PathBuf {
        self.home().join(".dir_layout")
    }
}

#[test]
fn create_dir_test() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let home = tempdir.path().join("x");
    let dir = Dir::new(&home).unwrap();
    assert!(dir.home().is_dir());
    assert!(dir.config().is_dir());
    assert!(dir.db().is_dir());
    assert!(dir.backups().is_dir());
    assert!(dir.dir_layout().is_file());
}

#[test]
fn create_dir_exists_test() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let home = tempdir.path().join("y");
    fs::create_dir_all(&home).unwrap();
    let dir = Dir::new(&home).unwrap();
    assert!(dir.home().is_dir());
    assert!(dir.config().is_dir());
    assert!(dir.db().is_dir());
    assert!(dir.backups().is_dir());
    assert!(dir.dir_layout().is_file());
}

#[test]
fn create_dir_bad_layout_version() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let home = tempdir.path().join("z");
    fs::create_dir_all(&home).unwrap();
    fs::write_all(home.join(".dir_layout"), "999999".bytes()).unwrap();
    assert!(Dir::new(&home).is_err());
}
