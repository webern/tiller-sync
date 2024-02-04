use anyhow::{Context, Result};
use std::io::{ErrorKind, Write};
use std::path::Path;

pub(crate) fn create_dir_all(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    match std::fs::create_dir_all(path) {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Ok(()),
            _ => Err(e).context(format!("Unable to create directory {}", path.display())),
        },
    }
}

// fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {

pub(crate) fn file(path: impl AsRef<Path>) -> Result<std::fs::File> {
    let path = path.as_ref();
    std::fs::File::create(path).context(format!("Unable to create file {}", path.display()))
}

pub(crate) fn write_all(path: impl AsRef<Path>, data: impl IntoIterator<Item = u8>) -> Result<()> {
    let path = path.as_ref();
    let mut f = file(path)?;
    let mut buf: Vec<u8> = data.into_iter().collect();
    f.write_all(buf.as_mut_slice())
        .context(format!("Unable to write data to {}", path.display()))
}

pub(crate) fn read_to_string(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    std::fs::read_to_string(path).context(format!("Unable to read file {}", path.display()))
}
