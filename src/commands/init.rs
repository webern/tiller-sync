use crate::commands::Out;
use crate::error::{ErrorType, IntoResult};
use crate::{Config, Result};
use anyhow::Context;
use std::path::Path;

/// Creates the data directory, its subdirectories and:
/// - Creates an initial `config.json` file using `sheet_url` along with default settings
/// - Copies `secret_file` into its default location in the data dir.
///
/// # Arguments
/// - `tiller_home` - The directory that will be the root of data directory, e.g. `$HOME/tiller`
/// - `secret_file` - The downloaded OAuth 2.0 client credentials JSON needed to start the Google
///   OAuth workflow. This will be copied from the `secret_file` path to its default location and
///   name in the data directory.
/// - `sheet_url` - The URL of the Google Sheet where the Tiller financial data is stored.
///   e.g.https://docs.google.com/spreadsheets/d/1a7Km9FxQwRbPt82JvN4LzYpH5OcGnWsT6iDuE3VhMjX
///
/// # Errors
/// - Returns an error if any file operations fail.
pub async fn init(tiller_home: &Path, secret_file: &Path, url: &str) -> Result<Out<()>> {
    let _config = Config::create(tiller_home, secret_file, url)
        .await
        .context("Unable to create the data directory and configs")
        .pub_result(ErrorType::Config)?;
    Ok("Successfully created the tiller directory and config".into())
}
