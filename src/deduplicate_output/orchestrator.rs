use anyhow::{Result, anyhow, bail};
use crossbeam_channel::{bounded, unbounded};
use std::fs::{read_dir, File};
use std::path::{Path, PathBuf};
use crate::deduplicate_output::chunk_buffer::ChunkBuffer;
use crate::deduplicate_output::threading::dedup_workers::WorkerResult;
use crate::deduplicate_output::threading::{
    spawn_compressor, spawn_dispatcher, spawn_worker, spawn_writers
};
use crate::shared::EntryBuffer;

const GIB: u64 = 1024 * 1024 * 1024;
const CHUNK_SIZE: usize = 256 * 1024;
const CHUNK_POOL_SIZE: usize = 8;

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

    // file buffer pool
    let (tx_file, rx_file) = bounded::<PathBuf>(worker_thread_count as usize * 2);
    let (tx_entry, rx_entry) = bounded::<WorkerResult>(channel_bin_input as usize);
    let (tx_buffer_empty, rx_buffer_empty) = unbounded::<EntryBuffer>();
    for _ in 0..channel_bin_input {
        tx_buffer_empty.send(EntryBuffer::with_capacity(max_file_size as usize))?;
    }

    // seq compressor channels
    let (tx_seq_chunk, rx_seq_chunk) = bounded::<ChunkBuffer>(CHUNK_POOL_SIZE);
    let (tx_seq_empty, rx_seq_empty) = bounded::<ChunkBuffer>(CHUNK_POOL_SIZE);
    for _ in 0..CHUNK_POOL_SIZE {
        tx_seq_empty.send(ChunkBuffer::with_capacity(CHUNK_SIZE))?;
    }

    // meta compressor channels
    let (tx_meta_chunk, rx_meta_chunk) = bounded::<ChunkBuffer>(CHUNK_POOL_SIZE);
    let (tx_meta_empty, rx_meta_empty) = bounded::<ChunkBuffer>(CHUNK_POOL_SIZE);
    for _ in 0..CHUNK_POOL_SIZE {
        tx_meta_empty.send(ChunkBuffer::with_capacity(CHUNK_SIZE))?;
    }

    std::fs::create_dir_all(outdir)?;
    let seq_file = File::create(outdir.join(if zip { "peptides.fasta.gz" } else { "peptides.fasta" }))?;
    let meta_file = File::create(outdir.join(if zip { "metadata.csv.gz" } else { "metadata.csv" }))?;

    let seq_compressor = spawn_compressor(rx_seq_chunk, tx_seq_empty, seq_file, zip);
    let meta_compressor = spawn_compressor(rx_meta_chunk, tx_meta_empty, meta_file, zip);
    let writer_handle = spawn_writers(
        rx_entry,
        tx_buffer_empty,
        tx_seq_chunk,
        tx_meta_chunk,
        rx_seq_empty,
        rx_meta_empty,
    );

    let mut worker_handles = Vec::with_capacity(worker_thread_count as usize);
    for _ in 0..worker_thread_count {
        let h = spawn_worker(rx_file.clone(), tx_entry.clone(), rx_buffer_empty.clone());
        worker_handles.push(h);
    }
    drop(tx_entry);

    let dispatcher_handle = spawn_dispatcher(indir, tx_file);
    dispatcher_handle
        .join()
        .map_err(|_| anyhow!("Dispatcher thread panicked"))??;
    for h in worker_handles {
        h.join().map_err(|_| anyhow!("Worker thread panicked"))??;
    }
    writer_handle
        .join()
        .map_err(|_| anyhow!("Writer thread panicked"))??;
    seq_compressor
        .join()
        .map_err(|_| anyhow!("Seq compressor thread panicked"))??;
    meta_compressor
        .join()
        .map_err(|_| anyhow!("Meta compressor thread panicked"))??;

    Ok(())
}

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