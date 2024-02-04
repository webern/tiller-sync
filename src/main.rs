use anyhow::Result;
use clap::Parser;
use fin::args::Fin;

fn main() -> Result<()> {
    let args = Fin::parse();
    fin::run(args)
}
