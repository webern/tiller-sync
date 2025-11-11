use crate::args::Args;
use crate::error::Result;
use clap::Parser;
use env_logger::Builder;
use log::{debug, error, trace, LevelFilter};
use std::process::ExitCode;

mod args;
mod error;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    init_logger(args.common.log_level);
    debug!("Log level set to {}", args.common.log_level);

    match main_inner(args).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!("Exiting with error: {e}");
            ExitCode::FAILURE
        }
    }
}

pub async fn main_inner(args: Args) -> Result<()> {
    trace!("{args:?}");
    Ok(())
}

/// Initializes the env_logger.
pub fn init_logger(level: LevelFilter) {
    match std::env::var(env_logger::DEFAULT_FILTER_ENV).ok() {
        Some(_) => {
            // RUST_LOG exists; env_logger will use it.
            Builder::from_default_env().init();
        }
        None => {
            // RUST_LOG does not exist; use default log level for this crate only.
            Builder::new()
                .filter(Some(env!("CARGO_CRATE_NAME")), level)
                .filter(Some(env!("CARGO_BIN_NAME")), level)
                .init();
        }
    }
}
