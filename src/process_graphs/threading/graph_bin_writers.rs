use std::{
    collections::HashMap,
    fs::File,
    io::{BufWriter, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

use anyhow::{Context, Error, Result};
use crossbeam_channel::Receiver;
use lru::LruCache;

use crate::process_graphs::io::bin_writer::{
    ensure_parent_dir, open_writer, resolve_path, write_entry_binary,
};
use crate::shared::BinEntry;

pub fn spawn_writer_manager(
    out_dir: &Path,
    hash_bits: Option<u8>,
    max_handles: Option<u32>,
    rx_entry: Receiver<BinEntry>,
) -> Result<JoinHandle<Result<Vec<PathBuf>, Error>>> {
    let out_dir = out_dir.to_path_buf();

    let handle =
        thread::spawn(move || writer_manager_thread(rx_entry, &out_dir, hash_bits, max_handles));

    Ok(handle)
}

fn writer_manager_thread(
    rx: Receiver<BinEntry>,
    out_dir: &Path,
    hash_bits: Option<u8>,
    max_handles: Option<u32>,
) -> Result<Vec<PathBuf>> {
    // determine maximum file handles if not set
    let max_h = max_handles.unwrap_or_else(get_sys_open_files);

    // determine hash bits if not set
    let h_bits = hash_bits.unwrap_or_else(|| hash_bits_for(max_h));

    let shard_mask = (1usize << h_bits) - 1;

    // enable directory fanout once 256 shards are surpassed
    let use_subdirs = h_bits > 8;

    // lru for file handles
    let mut writers: LruCache<PathBuf, BufWriter<File>> =
        LruCache::new(NonZeroUsize::new(max_h as usize).context("max_open_files must be > 0")?);

    // shard_id -> filename
    let mut filenames: HashMap<usize, PathBuf> = HashMap::new();

    let tmp_path = &out_dir.join("tmp");

    while let Ok(entry) = rx.recv() {
        let path = resolve_path(&entry, tmp_path, shard_mask, use_subdirs, &mut filenames);

        ensure_parent_dir(&path)?;

        let writer = get_writer(&mut writers, &path)?;

        write_entry_binary(writer, &entry)?;
    }

    // flush remaining handles
    for (_, writer) in writers.iter_mut() {
        writer.flush()?;
    }

    let mut files: Vec<PathBuf> = filenames.into_values().collect();

    files.sort();

    Ok(files)
}

fn get_writer<'a>(
    writers: &'a mut LruCache<PathBuf, BufWriter<File>>,
    path: &PathBuf,
) -> Result<&'a mut BufWriter<File>> {
    if !writers.contains(path) {
        let writer = open_writer(path)?;

        if let Some((_, mut evicted)) = writers.push(path.clone(), writer) {
            evicted.flush()?;
        }
    }

    writers
        .get_mut(path)
        .context("writer disappeared unexpectedly")
}

#[cfg(unix)]
fn get_sys_open_files() -> u32 {
    use nix::sys::resource::{Resource, getrlimit};

    // uses 80% of file handles
    match getrlimit(Resource::RLIMIT_NOFILE) {
        Ok((soft, _)) => (soft * 8 / 10).clamp(64, 8192) as u32,
        Err(_) => 512,
    }
}

#[cfg(windows)]
fn get_sys_open_files() -> u32 {
    2048
}

fn hash_bits_for(target: u32) -> u8 {
    let shards = target.next_power_of_two();
    shards.trailing_zeros() as u8
}
