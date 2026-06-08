use anyhow::{Result, anyhow, bail};
use crossbeam_channel::{bounded, unbounded};
use std::fs::read_dir;
use std::path::{Path, PathBuf};

use crate::deduplicate_output::threading::dedup_workers::WorkerResult;
use crate::deduplicate_output::threading::{spawn_dispatcher, spawn_worker, spawn_writers};
use crate::shared::EntryBuffer;

const GIB: u64 = 1024 * 1024 * 1024;

pub fn dedup_bin_files(
    num_threads: u64,
    avail_memory: u64,
    outdir: &Path,
    indir: PathBuf,
    zip: bool,
) -> Result<()> {
    let max_file_size = max_file_size(&indir)?;

    let mem_in_gib = avail_memory * GIB;

    let memory_budget_entry = mem_in_gib * 70 / 100;

    if max_file_size > memory_budget_entry {
        bail!(
            "Files do not fit to memory (max file: {} bytes, budget: {} bytes)",
            max_file_size,
            memory_budget_entry
        );
    }

    let channel_bin_input = (memory_budget_entry / max_file_size).min(num_threads * 2);

    let worker_thread_count = num_threads.min(channel_bin_input);

    let (tx_file, rx_file) = bounded::<PathBuf>(worker_thread_count as usize * 2);
    let (tx_entry, rx_entry) = bounded::<WorkerResult>(channel_bin_input as usize);
    let (tx_buffer_empty, rx_buffer_empty) = unbounded::<EntryBuffer>();

    for _ in 0..channel_bin_input {
        tx_buffer_empty.send(EntryBuffer::with_capacity(max_file_size as usize))?;
    }

    let writer_handle = spawn_writers(rx_entry, tx_buffer_empty, outdir, zip);

    let mut worker_handles = Vec::with_capacity(worker_thread_count as usize);

    for _ in 0..worker_thread_count {
        let h = spawn_worker(rx_file.clone(), tx_entry.clone(), rx_buffer_empty.clone());
        worker_handles.push(h);
    }

    drop(tx_entry);

    let dispatcher_handles = spawn_dispatcher(indir, tx_file);

    dispatcher_handles
        .join()
        .map_err(|_| anyhow!("dispatcher panicked"))??;

    for h in worker_handles {
        h.join().map_err(|_| anyhow!("Worker thread panicked"))??;
    }

    writer_handle
        .join()
        .map_err(|_| anyhow!("Writer thread panicked"))??;

    Ok(())
}

/// Scan directory and return maximum file size in bytes
fn max_file_size(indir: &PathBuf) -> Result<u64> {
    let mut max_size = 0u64;
    let mut found = false;

    for entry in read_dir(indir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            found = true;
            let size = entry.metadata()?.len();
            max_size = max_size.max(size);
        }
    }

    if !found {
        bail!("No input files found in directory: {}", indir.display());
    }

    Ok(max_size)
}
