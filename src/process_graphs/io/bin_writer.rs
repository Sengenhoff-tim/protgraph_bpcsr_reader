use std::{
    fs::{File, OpenOptions},
    io::{BufWriter},
    path::Path,
};

use anyhow::Result;

/// Opens (or creates) a shard file for appending, buffered.
pub fn open_writer(path: &Path) -> Result<BufWriter<File>> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    Ok(BufWriter::new(file))
}