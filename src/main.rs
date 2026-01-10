use clap::Parser;
use std::process::ExitCode;
use tiller_sync::args::{
    Args, Command, DeleteSubcommand, InsertSubcommand, UpDown, UpdateSubcommand,
};
use tiller_sync::{commands, Config, Mode, Result};
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
            error!("Exiting with error: {e}");
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
    let _: () = match args.command() {
        Command::Init(init_args) => {
            commands::init(home, init_args.client_secret(), init_args.sheet_url())
                .await?
                .print()
        }

        Command::Auth(auth_args) => {
            let config = Config::load(home).await?;
            if auth_args.verify() {
                commands::auth_verify(&config).await?.print()
            } else {
                commands::auth(&config).await?.print()
            }
        }

        Command::Sync(sync_args) => {
            let config = Config::load(home).await?;
            match sync_args.direction() {
                UpDown::Up => {
                    commands::sync_up(config, mode, sync_args.force(), sync_args.formulas())
                        .await?
                        .print()
                }
                UpDown::Down => commands::sync_down(config, mode).await?.print(),
            }
        }

        Command::Mcp(_mcp_args) => commands::mcp(Config::load(home).await?, mode)
            .await?
            .print(),

        Command::Update(update_args) => {
            let config = Config::load(home).await?;
            match update_args.entity() {
                UpdateSubcommand::Transactions(args) => {
                    commands::update_transactions(config, *args.clone())
                        .await?
                        .print()
                }
                UpdateSubcommand::Categories(args) => {
                    commands::update_categories(config, args.clone())
                        .await?
                        .print()
                }
                UpdateSubcommand::Autocats(args) => commands::update_autocats(config, args.clone())
                    .await?
                    .print(),
            }
        }

        Command::Delete(delete_args) => {
            let config = Config::load(home).await?;
            match delete_args.entity() {
                DeleteSubcommand::Transactions(args) => {
                    commands::delete_transactions(config, args.clone())
                        .await?
                        .print()
                }
                DeleteSubcommand::Categories(args) => {
                    commands::delete_categories(config, args.clone())
                        .await?
                        .print()
                }
                DeleteSubcommand::Autocats(args) => commands::delete_autocats(config, args.clone())
                    .await?
                    .print(),
            }
        }

        Command::Insert(insert_args) => {
            let config = Config::load(home).await?;
            match insert_args.entity() {
                InsertSubcommand::Transaction(args) => {
                    commands::insert_transaction(config, *args.clone())
                        .await?
                        .print()
                }
                InsertSubcommand::Category(args) => commands::insert_category(config, args.clone())
                    .await?
                    .print(),
                InsertSubcommand::Autocat(args) => commands::insert_autocat(config, *args.clone())
                    .await?
                    .print(),
            }
        }

        Command::Query(query_args) => {
            let config = Config::load(home).await?;
            commands::query(config, query_args.clone()).await?.print()
        }

        Command::Schema(schema_args) => {
            let config = Config::load(home).await?;
            commands::schema(config, schema_args.clone()).await?.print()
        }
    };
    Ok(())
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
