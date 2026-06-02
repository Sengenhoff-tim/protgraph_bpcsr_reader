use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::Result;
use bincode::{config::standard, encode_to_vec};
use xxhash_rust::xxh64::xxh64;

use crate::shared::BinEntry;

const SEED: u64 = 0xC0111DE;

pub fn write_entry_binary(writer: &mut BufWriter<File>, entry: &BinEntry) -> Result<()> {
    let bytes = encode_to_vec(entry, standard())?;

    let len = bytes.len() as u32;

    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&bytes)?;

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

/// Creates binary files with hash as filename. Optionally creates subdirs based on hash as well.
pub fn shard_filename(out_dir: &Path, hash: u64, shard_id: usize, use_subdirs: bool) -> PathBuf {
    let filename = format!("{shard_id:05x}.bin");

    if use_subdirs {
        let dir = format!("{:02x}", (hash >> 56) & 0xff);

        out_dir.join(dir).join(filename)
    } else {
        out_dir.join(filename)
    }
}

pub fn resolve_path(
    entry: &BinEntry,
    out_dir: &Path,
    shard_mask: usize,
    use_subdirs: bool,
    filenames: &mut HashMap<usize, PathBuf>,
) -> PathBuf {
    let hash = xxh64(entry.seq.as_bytes(), SEED);
    let shard_id = (hash as usize) & shard_mask;

    filenames
        .entry(shard_id)
        .or_insert_with(|| shard_filename(out_dir, hash, shard_id, use_subdirs))
        .clone()
}
