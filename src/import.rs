use crate::error::Re;
use crate::model::CsvRecord;
use crate::Args;
use anyhow::Context;
use clap::Parser;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Boop Beep
#[derive(Debug, Parser)]
pub(crate) struct Parse {
    /// The file to read. If not supplied, input will be taken from stdin.
    #[clap(long = "file", short = 'f')]
    file: Option<PathBuf>,
}

impl Parse {
    pub(crate) fn run(&self, _args: &Args) -> Re<()> {
        let r: Box<dyn BufRead> = match &self.file {
            None => Box::new(BufReader::new(io::stdin())),
            Some(path) => {
                let f = std::fs::File::open(path)
                    .context(format!("unable to open file {}", path.display()))?;
                Box::new(BufReader::new(f))
            }
        };
        let mut rdr = csv::Reader::from_reader(r);
        let mut records = Vec::new();
        for result in rdr.deserialize() {
            let record: CsvRecord = result?;
            records.push(record);
        }

        Ok(())
    }
}
