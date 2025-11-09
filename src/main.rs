use crate::args::Args;
use crate::error::Result;
use clap::Parser;
use log::error;
use std::process::ExitCode;

mod args;
mod error;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    match main_inner(args).await {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!("Exiting with error: {e}");
            ExitCode::FAILURE
        }
    }
}

pub async fn main_inner(args: Args) -> Result<()> {
    println!("{args:?}");
    Ok(())
}
