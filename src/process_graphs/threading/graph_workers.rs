use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use xxhash_rust::xxh64::xxh64;

use crate::process_graphs::{utilities::{TraversalContext, TraversalJob}};
use crate::shared::{MemBuffer, Record};

const SEED: u64 = 0x1203012301203;

pub fn spawn_workers(
    rx_jobs: Receiver<TraversalJob>,
    rx_reuse: Receiver<MemBuffer>,
    txs_filled: Vec<Sender<MemBuffer>>,
    batch_target_size: usize,
    num_threads: usize,
    ctx: Arc<TraversalContext>,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::with_capacity(num_threads);

    for i in 0..num_threads {
        let rx = rx_jobs.clone();
        let rx_reuse = rx_reuse.clone();
        let txs_filled = txs_filled.clone();
        let ctx = Arc::clone(&ctx);

        let handle = thread::spawn(move || {
            traversal_thread(rx, rx_reuse, txs_filled, batch_target_size, ctx)
                .with_context(|| format!("worker {i} died"))
        });

        handles.push(handle);
    }

    Ok(handles)
}

fn traversal_thread(
    rx: Receiver<TraversalJob>,
    rx_reuse: Receiver<MemBuffer>,
    txs_filled: Vec<Sender<MemBuffer>>,
    batch_target_size: usize,
    ctx: Arc<TraversalContext>,
) -> Result<()> {
    let n_writers = txs_filled.len();
    let mut batch_buffers: Vec<MemBuffer> = (0..n_writers)
        .map(|_| rx_reuse.recv())
        .collect::<Result<_, _>>()?;

    let mut sequence_buffer: Vec<u8> = Vec::new();

    while let Ok(mut job) = rx.recv() {
        let graph = &job.graph;
        job.graph.tree_data.traverse_depth_first(
            &ctx,
            &mut job.path,
            &mut |path| {
                graph.meta_data.build_sequence(path, &mut sequence_buffer)?;

                let h = xxh64(&sequence_buffer, SEED);
                let idx = (h % n_writers as u64) as usize;

                let record = Record {
                    metas: graph.meta_data.build_metas(path)?,
                    sequence: sequence_buffer.clone(),
                    hash: h,
                };
                batch_buffers[idx].push(record);

                flush_if_full(
                    &mut batch_buffers[idx],
                    batch_target_size,
                    &rx_reuse,
                    &txs_filled[idx],
                )
            },
        )?;
    }

    for (idx, buf) in batch_buffers.into_iter().enumerate() {
        if !buf.is_empty() {
            txs_filled[idx].send(buf)?;
        }
    }

    Ok(())
}

fn flush_if_full(
    batch_buffer: &mut MemBuffer,
    batch_target_size: usize,
    rx_reuse: &Receiver<MemBuffer>,
    tx_filled: &Sender<MemBuffer>,
) -> Result<()> {
    if batch_buffer.byte_len() >= batch_target_size {
        let new_buffer = rx_reuse.recv()?;
        let old_buffer = std::mem::replace(batch_buffer, new_buffer);
        tx_filled.send(old_buffer)?;
    }
    Ok(())
}