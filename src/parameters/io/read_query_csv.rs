use std::{fs::File, path::PathBuf};

use anyhow::Result;
use csv::Reader;
use serde::Deserialize;

use crate::process_graphs::utilities::{Interval, WEIGHT_FACTOR};

#[derive(Deserialize)]
struct RawInterval {
    lower: f64,
    upper: f64,
}

pub fn read_query_csv(path: &PathBuf, lower: i64, upper: i64) -> Result<Vec<Interval>> {
    let file = File::open(path)?;
    let mut reader = Reader::from_reader(file);

    let intervals: Vec<Interval> = reader
        .deserialize()
        .map(|result| {
            let raw: RawInterval = result?;

            let mut interval = Interval {
                lower: (raw.lower * WEIGHT_FACTOR as f64).round() as i64,
                upper: (raw.upper * WEIGHT_FACTOR as f64).round() as i64,
            };

            interval.clamp(lower, upper);

            interval.validate()?;

            Ok(interval)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(intervals)
}