use clap::Parser;
use env_logger::Builder;
use log::{debug, error, trace, LevelFilter};
use std::process::ExitCode;
use tiller_sync::args::{Args, Command};
use tiller_sync::commands;
use tiller_sync::{Config, Result};

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let log_level = args.common().log_level();
    init_logger(log_level);
    debug!("Log level set to {}", log_level.to_string().to_lowercase());

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

    // Initialize home directory
    let config = Config::new(args.common().tiller_home().path()).await?;

    // Route to appropriate command handler
    match args.command() {
        Command::Auth(auth_args) => {
            if auth_args.verify() {
                commands::handle_auth_verify(&config).await?;
            } else {
                commands::handle_auth_command(&config).await?;
            }
        }
        Command::Sync(_sync_args) => {
            // TODO: Implement sync command
            unimplemented!("Sync command not yet implemented");
        }
    }

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
