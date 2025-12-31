use clap::Parser;
use std::process::ExitCode;
use tiller_sync::args::{Args, Command, UpDown};
use tiller_sync::{commands, Mode};
use tiller_sync::{Config, Result};
use tracing::{debug, error, trace};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let log_level = args.common().log_level();
    init_logger(log_level);
    debug!("Log level set to {}", log_level.to_string().to_lowercase());

    match main_inner(args).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!("Exiting with error: {e:?}");
            ExitCode::FAILURE
        }
    }
}

pub async fn main_inner(args: Args) -> Result<()> {
    trace!("{args:?}");
    let home = args.common().tiller_home().path();

    // This allows for testing the program without hitting the Google APIs. When
    // TILLER_SYNC_IN_TEST_MODE is set and non-zero in length, then the mode will be Mode::Test,
    // otherwise it will be Mode::Google.
    let mode = Mode::from_env();

    // Route to appropriate command handler
    match args.command() {
        Command::Init(init_args) => {
            commands::init(home, init_args.client_secret(), init_args.sheet_url()).await
        }
        Command::Auth(auth_args) => {
            let config = Config::load(home).await?;
            if auth_args.verify() {
                commands::auth_verify(&config).await
            } else {
                commands::auth(&config).await
            }
        }
        Command::Sync(sync_args) => match sync_args.direction() {
            UpDown::Up => {
                let formulas_mode = commands::FormulasMode::Unknown;
                commands::sync_up(
                    Config::load(home).await?,
                    mode,
                    sync_args.force(),
                    formulas_mode,
                )
                .await
            }
            UpDown::Down => commands::sync_down(Config::load(home).await?, mode).await,
        },
    }
}

/// Initializes the tracing subscriber.
pub fn init_logger(level: LevelFilter) {
    let filter = match std::env::var("RUST_LOG").ok() {
        Some(_) => {
            // RUST_LOG exists; use it.
            EnvFilter::from_default_env()
        }
        None => {
            // RUST_LOG does not exist; use default log level for this crate only.
            EnvFilter::new(format!(
                "{}={},{}={}",
                env!("CARGO_CRATE_NAME"),
                level,
                env!("CARGO_BIN_NAME"),
                level
            ))
        }
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}
