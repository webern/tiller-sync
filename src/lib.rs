#![allow(unused)]

pub mod args;
mod dir;
mod fs;

use crate::args::Fin;
use anyhow::Result;

pub fn run(args: Fin) -> Result<()> {
    println!("{:?}", args);
    Ok(())
}
