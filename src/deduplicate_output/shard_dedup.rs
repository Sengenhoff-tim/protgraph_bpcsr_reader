//! Final dedup of the shard files of all intervals that spilled.

use std::{fs::read_dir, path::PathBuf, thread};

use anyhow::{Result, anyhow, bail};
use crossbeam_channel::{Sender, bounded};
use tracing::{info, warn};

use crate::{deduplicate_output::threading::shard_worker, shared::OutputBatch};

/// Rough ratio of decompressed shard plus grouping overhead to compressed file size.
const EXPANSION_ESTIMATE: u64 = 4;

/// Shard files (`*.zst`) of the given interval directories (`dir/w*/shard_*.zst`).
fn shard_files(dirs: &[PathBuf]) -> Result<Vec<(PathBuf, u64)>> {
    let mut files = Vec::new();
    for dir in dirs {
        for writer_dir in read_dir(dir)? {
            let writer_dir = writer_dir?.path();
            if !writer_dir.is_dir() {
                continue;
            }
            for shard in read_dir(&writer_dir)? {
                let shard = shard?;
                let path = shard.path();
                if path.is_file() && path.extension().is_some_and(|e| e == "zst") {
                    files.push((path, shard.metadata()?.len()));
                }
            }
        }
    }
    Ok(files)
}

/// Deduplicates every shard file under `dirs` and sends the results to the output.
pub fn dedup_spilled(
    dirs: &[PathBuf],
    threads: usize,
    memory_budget: u64,
    acc_width: usize,
    tx_out: Sender<OutputBatch>,
) -> Result<()> {
    let mut files = shard_files(dirs)?;
    if files.is_empty() {
        return Ok(());
    }
    // largest first, so the most memory hungry shards don't end up running alone at the end
    files.sort_by_key(|f| std::cmp::Reverse(f.1));

    let max_file = files[0].1;
    if max_file > memory_budget {
        bail!(
            "shard {} ({max_file} bytes) does not fit into memory ({memory_budget} bytes)",
            files[0].0.display()
        );
    }
    let per_worker = (max_file * EXPANSION_ESTIMATE).max(1);
    let workers = ((memory_budget / per_worker) as usize).clamp(1, threads.max(1));
    if per_worker > memory_budget {
        warn!("largest shard may not fit into memory after decompression; using one worker");
    }
    info!("DEDUP {} shard files with {} workers", files.len(), workers);

    let (tx_paths, rx_paths) = bounded::<PathBuf>(workers * 2);
    let handles: Vec<_> = (0..workers)
        .map(|_| {
            let (rx, tx) = (rx_paths.clone(), tx_out.clone());
            thread::spawn(move || shard_worker(rx, tx, acc_width))
        })
        .collect();
    drop((rx_paths, tx_out));

    let mut send_err = None;
    for (path, _) in files {
        if let Err(e) = tx_paths.send(path) {
            send_err = Some(anyhow!("shard workers stopped: {e}"));
            break;
        }
    }
    drop(tx_paths);

    for handle in handles {
        handle
            .join()
            .map_err(|_| anyhow!("shard worker panicked"))??;
    }
    send_err.map_or(Ok(()), Err)
}
