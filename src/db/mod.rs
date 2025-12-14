//! This module is responsible for reading, writing and managing the SQLite database

use crate::Result;
use std::path::Path;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Db {
    // TODO: add the actual SQLite library
    _sql_client: (),
}

impl Db {
    /// - Validates that there is a SQLite file at `path`
    /// - Creates a SQLite client
    /// - Updates the database schema with migrations if it is out-of-date
    /// - Returns a constructed `Datastore` object for further operations
    pub(crate) fn load(_path: impl AsRef<Path>) -> Result<Self> {
        // TODO: validate the SQLite file exists and that the schema is valid. Run migrations.
        Ok(Self { _sql_client: () })
    }

    /// - Validates that no file currently exists at `path`
    /// - Creates a new SQLite file at `path`
    /// - Initializes the database schema
    /// - Returns a constructed `Datastore` object for further operations
    pub(crate) fn init(_path: impl AsRef<Path>) -> Result<Self> {
        // TODO: create the initial SQLite file (error if one exists) and initialize the schema.
        Ok(Self { _sql_client: () })
    }

    /// - Returns the number of rows in the transactions table
    pub(crate) fn count_transactions(&self) -> Result<u64> {
        // TODO: return the actual count of transaction rows
        Ok(100)
    }
}
