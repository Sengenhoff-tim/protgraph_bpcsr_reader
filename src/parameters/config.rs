use anyhow::{
    Result,
};
use clap::Parser;

use crate::parameters::Cli;
use crate::parameters::io::read_query_csv::read_query_csv;
use crate::process_graphs::utilities::{Interval, IntervalVecExt};

/// struct containing the collated input
pub struct Config {
    pub cli: Cli,
    pub intervals: Box<[Interval]>,
}

impl Config {
    pub fn new() -> Result<Config> {
        let cli = Cli::parse();

        // queries are read from query input csv in Da
        let intervals = read_query_csv(&cli.query_input_path, cli.lower_bound, cli.upper_bound)?;

        // intervals are sorted, adjacent intervals are merged
        let normalized = intervals.merge_and_sort()?;

        Ok(Config {
            cli,
            intervals: normalized,
        })
    }
}
