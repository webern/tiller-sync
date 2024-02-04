pub mod args;

use crate::args::Fin;
use anyhow::Result;

pub fn run(args: Fin) -> Result<()> {
    println!("{:?}", args);
    Ok(())
}
