use std::{fs::File, path::PathBuf};

use anyhow::{Result};
use csv::Reader;

use crate::process_graphs::utilities::Interval;

pub fn read_query_csv(
    path: &PathBuf,
    lower: i64,
    upper: i64,
) -> Result<Vec<Interval>> {
    let file = File::open(path)?;
    let mut reader = Reader::from_reader(file);

    let intervals: Vec<Interval> = reader
        .deserialize()
        .map(|result| {
            let mut interval: Interval = result?;

            interval
                .clamp(lower, upper)
                .apply_weight_factor();

            interval.validate()?;

            Ok(interval)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(intervals)
}


