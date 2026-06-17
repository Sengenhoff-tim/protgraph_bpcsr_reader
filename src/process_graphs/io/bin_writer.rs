use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::Result;
use xxhash_rust::xxh64::xxh64;

const SEED: u64 = 0xC0111DE;

pub fn write_entry_binary(writer: &mut BufWriter<File>, entry_bytes: &[u8]) -> Result<()> {
    writer.write_all(entry_bytes)?;
    Ok(())
}

pub fn open_writer(path: &Path) -> Result<BufWriter<File>> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(path)?;

    Ok(BufWriter::new(file))
}

/// Creates binary files with hash as filename.
pub fn shard_filename(out_dir: &Path, shard_id: usize) -> PathBuf {
    let filename = format!("{shard_id:05x}.bin");
    out_dir.join(filename)
}

pub fn resolve_path(
    entry: &[u8],
    out_dir: &Path,
    shard_mask: usize,
    filenames: &mut HashMap<usize, PathBuf>,
) -> PathBuf {
    let hash = xxh64(entry, SEED);
    let shard_id = (hash as usize) & shard_mask;
    filenames
        .entry(shard_id)
        .or_insert_with(|| shard_filename(out_dir, shard_id))
        .clone()
}