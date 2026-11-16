use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender, select};
use xxhash_rust::xxh64::xxh64;

use crate::{process_graphs::utilities::TraversalJob, shared::MemBuffer};

use super::WriterMsg;

pub const SEED: u64 = 0x1203012301203;

/// Asks a worker to send its partial batches; it answers on the contained sender.
pub type FlushRequest = Sender<()>;

/// Routes a sequence hash to its writer. Every copy of a sequence goes to the same writer.
#[inline]
pub fn writer_of(hash: u64, n_writers: usize) -> usize {
    (hash % n_writers as u64) as usize
}

/// Traverses jobs and batches the results per writer. A message on `rx_flush` makes the
/// worker send all partially filled batches and then acknowledge on the given sender;
/// the coordinator uses this at the end of each interval.
pub fn worker_loop(
    rx_jobs: Receiver<TraversalJob>,
    rx_flush: Receiver<FlushRequest>,
    rx_reuse: Receiver<MemBuffer>,
    txs_writer: Vec<Sender<WriterMsg>>,
    batch_target_size: usize,
) -> Result<()> {
    let n_writers = txs_writer.len();
    let mut batches: Vec<MemBuffer> = (0..n_writers)
        .map(|_| rx_reuse.recv())
        .collect::<Result<_, _>>()?;
    let mut sequence_buffer: Vec<u8> = Vec::new();

    loop {
        select! {
            recv(rx_jobs) -> job => {
                let Ok(mut job) = job else { break };
                let graph = &job.graph;
                let acc = graph.acc_id();
                graph.traverse_depth_first(&job.pass.ctx, &mut job.path, &mut |path| {
                    graph.build_sequence(path, &mut sequence_buffer);
                    let hash = xxh64(&sequence_buffer, SEED);
                    let idx = writer_of(hash, n_writers);
                    batches[idx].push(hash, acc, &sequence_buffer);
                    if batches[idx].byte_len() >= batch_target_size {
                        send_batch(&mut batches[idx], &rx_reuse, &txs_writer[idx])?;
                    }
                    Ok(())
                })?;
            }
            recv(rx_flush) -> ack => {
                let Ok(ack) = ack else { break };
                for (batch, tx) in batches.iter_mut().zip(&txs_writer) {
                    if !batch.is_empty() {
                        send_batch(batch, &rx_reuse, tx)?;
                    }
                }
                ack.send(()).context("coordinator gone")?;
            }
        }
    }

    for (batch, tx) in batches.into_iter().zip(&txs_writer) {
        if !batch.is_empty() {
            tx.send(WriterMsg::Batch(batch))?;
        }
    }
    Ok(())
}

fn send_batch(
    batch: &mut MemBuffer,
    rx_reuse: &Receiver<MemBuffer>,
    tx: &Sender<WriterMsg>,
) -> Result<()> {
    let empty = rx_reuse.recv().context("buffer pool closed")?;
    let full = std::mem::replace(batch, empty);
    tx.send(WriterMsg::Batch(full)).context("writer gone")?;
    Ok(())
}
