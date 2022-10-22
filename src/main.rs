mod error;
mod import;
mod model;

use crate::error::Re;
use anyhow::Context;
use clap::Parser;
use colored::Colorize;
use log::{trace, Level, LevelFilter};
use std::fs::create_dir_all;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub(crate) struct Args {
    /// Set logging verbosity [trace|debug|info|warn|error]. If the environment variable `RUST_LOG`
    /// is present, it overrides the default logging behavior. See https://docs.rs/env_logger/latest
    #[clap(long = "log-level", default_value = "warn")]
    pub(crate) log_level: LevelFilter,
    #[clap(long, short = 'd', env = "MINT_DATA_DIRECTORY", default_value_t=default_dir())]
    pub(crate) data_directory: String,
    #[clap(subcommand)]
    pub(crate) command: MainCommand,
}

#[derive(Debug, Parser)]
pub(crate) enum MainCommand {
    /// Parses a downloaded mint transactions.csv file into JSON
    Parse(import::Parse),
}

pub(crate) fn run(args: &Args) -> Re<()> {
    create_dir_all(&args.data_directory).context(format!(
        "unable to create directory '{}'",
        args.data_directory
    ))?;
    match &args.command {
        MainCommand::Parse(x) => x.run(args),
    }
}

fn main() {
    let args = Args::parse();
    init_logger(args.log_level);
    trace!("tracing enabled");
    if let Err(e) = run(&args) {
        eprintln!("{}", e);
        std::process::exit(1)
    }
}

fn init_logger(level: LevelFilter) {
    // extract the value of RUST_LOG if it exists
    let env_val = std::env::var(env_logger::DEFAULT_FILTER_ENV)
        .ok()
        .unwrap_or("".into());

    // create a builder from env if RUST_LOG exists, otherwise create a default builder
    let mut builder = if env_val.is_empty() {
        env_logger::Builder::new()
    } else {
        env_logger::Builder::from_default_env()
    };

    // set a default filter unless the user has passed a filter using the RUST_LOG env var
    if env_val.is_empty() {
        builder.filter(Some(env!("CARGO_CRATE_NAME")), level);
    }

    // if the user has passed RUST_LOG_STYLE, use it, otherwise use our favorite style
    if let Some(style_env_value) = std::env::var(env_logger::DEFAULT_WRITE_STYLE_ENV).ok() {
        builder.parse_write_style(&style_env_value);
    } else {
        builder.format(|buf, record| {
            let level = format!("[{}]", record.level());
            let level = match record.level() {
                Level::Error => level.red(),
                Level::Warn => level.yellow(),
                Level::Info => level.cyan(),
                Level::Debug => level.truecolor(100, 100, 100),
                Level::Trace => level.truecolor(200, 200, 200),
            };
            writeln!(
                buf,
                "{}:{} {} {} - {}",
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
                level,
                record.args(),
            )
        });
    }
    builder.init()
}

fn default_dir() -> String {
    let home: PathBuf = match dirs::home_dir() {
        Some(home) => home,
        None => PathBuf::from("unknown/path/to/home/"),
    }
    .join(".mint");
    format!("{}", home.display())
}
