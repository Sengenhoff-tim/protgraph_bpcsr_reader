//! Spawning the pipeline stages, last stage first so every stage gets its receivers.

use std::{path::Path, sync::Arc};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use super::sizing::Sizing;
use crate::{
    process_graphs::{
        io::arena_io::ArenaFile,
        threading::{
            FlushRequest, Spiller, Supervisor, Writer, WriterMsg, dispatcher_loop, reader_loop,
            worker_loop,
        },
        utilities::{GraphMsg, Pass, TraversalJob},
    },
    shared::{DedupTable, MemBuffer, OutputBatch, TableStats},
};

/// Pool of batch buffers shared by workers (take empty ones) and writers (return them).
/// Its size bounds the batches in flight: workers block while writers are behind.
pub(super) fn batch_pool(sizing: &Sizing) -> Result<(Sender<MemBuffer>, Receiver<MemBuffer>)> {
    let (tx, rx) = bounded::<MemBuffer>(sizing.pool_size);
    for _ in 0..sizing.pool_size {
        tx.send(MemBuffer::with_capacity(sizing.batch_bytes))?;
    }
    Ok((tx, rx))
}

/// One writer per share of the hash space. Returns the writers' inboxes.
pub(super) fn spawn_writers(
    sup: &mut Supervisor,
    sizing: &Sizing,
    tmp_dir: &Path,
    tx_out: &Sender<OutputBatch>,
    tx_reuse: &Sender<MemBuffer>,
) -> Result<Vec<Sender<WriterMsg>>> {
    (0..sizing.n_writers)
        .map(|idx| {
            let (tx, rx) = unbounded::<WriterMsg>();
            let writer = Writer {
                idx,
                table: DedupTable::new(sizing.table),
                spiller: Spiller::new(
                    tmp_dir,
                    idx,
                    sizing.n_writers,
                    sizing.n_shards,
                    sizing.spill_buf,
                    sizing.table.acc_width,
                ),
                tx_reuse: tx_reuse.clone(),
                tx_out: tx_out.clone(),
                stats_before: TableStats::default(),
            };
            sup.spawn(format!("writer {idx}"), move || writer.run(rx))?;
            Ok(tx)
        })
        .collect()
}

/// Workers traverse jobs. Returns the job sender and one flush channel per worker.
pub(super) fn spawn_workers(
    sup: &mut Supervisor,
    sizing: &Sizing,
    txs_writer: &[Sender<WriterMsg>],
    rx_reuse: &Receiver<MemBuffer>,
) -> Result<(Sender<TraversalJob>, Vec<Sender<FlushRequest>>)> {
    let (tx_jobs, rx_jobs) = bounded::<TraversalJob>(sizing.queue_depth);
    let txs_flush = (0..sizing.threads)
        .map(|idx| {
            let (tx_flush, rx_flush) = unbounded::<FlushRequest>();
            let (rx_jobs, rx_reuse, txs_writer) =
                (rx_jobs.clone(), rx_reuse.clone(), txs_writer.to_vec());
            let batch_bytes = sizing.batch_bytes;
            sup.spawn(format!("worker {idx}"), move || {
                worker_loop(rx_jobs, rx_flush, rx_reuse, txs_writer, batch_bytes)
            })?;
            Ok(tx_flush)
        })
        .collect::<Result<_>>()?;
    Ok((tx_jobs, txs_flush))
}

/// Dispatchers split graphs into jobs. Returns the graph sender.
pub(super) fn spawn_dispatchers(
    sup: &mut Supervisor,
    sizing: &Sizing,
    jobs_per_graph: usize,
    tx_jobs: Sender<TraversalJob>,
) -> Result<Sender<GraphMsg>> {
    let (tx_graphs, rx_graphs) = bounded::<GraphMsg>(sizing.queue_depth);
    for idx in 0..sizing.threads {
        let (rx_graphs, tx_jobs) = (rx_graphs.clone(), tx_jobs.clone());
        sup.spawn(format!("dispatcher {idx}"), move || {
            dispatcher_loop(rx_graphs, tx_jobs, jobs_per_graph)
        })?;
    }
    Ok(tx_graphs)
}

/// The reader reads all graphs once per pass. Returns the pass sender.
pub(super) fn spawn_reader(
    sup: &mut Supervisor,
    arena: Arc<ArenaFile>,
    tx_graphs: Sender<GraphMsg>,
) -> Result<Sender<Arc<Pass>>> {
    let (tx_pass, rx_pass) = unbounded::<Arc<Pass>>();
    sup.spawn("reader".into(), move || {
        reader_loop(arena, rx_pass, tx_graphs)
    })?;
    Ok(tx_pass)
}
