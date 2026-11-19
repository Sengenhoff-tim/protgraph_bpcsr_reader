use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, ensure};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use super::{PassReport, ProcessParams, sizing::Sizing, stages};
use crate::{
    process_graphs::{
        graph::MAX_VARIANTS,
        io::arena_io::ArenaFile,
        threading::{FlushRequest, Spiller, Supervisor, WriterMsg, WriterReport},
        utilities::{Interval, Pass, TraversalContext},
    },
    shared::OutputBatch,
};

/// Coordinator of the long-lived traversal threads.
pub struct ProcessPipeline {
    tmp_dir: PathBuf,
    max_vars: u16,
    max_cleavages: u16,
    supervisor: Supervisor,
    tx_pass: Sender<Arc<Pass>>,
    /// a pass reports its index here once all its graphs are traversed
    done_tx: Sender<usize>,
    done_rx: Receiver<usize>,
    txs_flush: Vec<Sender<FlushRequest>>,
    txs_writer: Vec<Sender<WriterMsg>>,
}

impl ProcessPipeline {
    pub fn spawn(
        params: &ProcessParams,
        arena: Arc<ArenaFile>,
        tmp_dir: &Path,
        tx_out: Sender<OutputBatch>,
        acc_width: usize,
    ) -> Result<Self> {
        ensure!(
            params.max_vars <= MAX_VARIANTS,
            "max_vars {} exceeds the supported maximum of {MAX_VARIANTS}",
            params.max_vars
        );
        let sizing = Sizing::new(params, acc_width);
        sizing.log();

        let mut supervisor = Supervisor::default();
        let (tx_reuse, rx_reuse) = stages::batch_pool(&sizing)?;
        let txs_writer =
            stages::spawn_writers(&mut supervisor, &sizing, tmp_dir, &tx_out, &tx_reuse)?;
        let (tx_jobs, txs_flush) =
            stages::spawn_workers(&mut supervisor, &sizing, &txs_writer, &rx_reuse)?;
        let tx_graphs =
            stages::spawn_dispatchers(&mut supervisor, &sizing, params.jobs_per_graph, tx_jobs)?;
        let tx_pass = stages::spawn_reader(&mut supervisor, arena, tx_graphs)?;
        let (done_tx, done_rx) = unbounded();

        Ok(Self {
            tmp_dir: tmp_dir.to_path_buf(),
            max_vars: params.max_vars,
            max_cleavages: params.max_cleavages,
            supervisor,
            tx_pass,
            done_tx,
            done_rx,
            txs_flush,
            txs_writer,
        })
    }

    /// Directory of the shard files of pass `idx` (exists only if the report says spilled).
    pub fn pass_dir(&self, idx: usize) -> PathBuf {
        Spiller::pass_dir(&self.tmp_dir, idx)
    }

    /// Traverses all graphs against `intervals` (sorted and merged) and finishes the pass in
    /// every writer. Passes must be run with consecutive indices starting at 0, and a
    /// sequence must not match intervals of two different passes.
    pub fn run_pass(&self, idx: usize, intervals: &[Interval]) -> Result<PassReport> {
        self.start_pass(idx, intervals)?;
        self.wait_traversed(idx)?;
        self.flush_workers()?;
        self.finish_writers()
    }

    /// Hands the reader a new pass; it re-reads all graphs for it.
    fn start_pass(&self, idx: usize, intervals: &[Interval]) -> Result<()> {
        let ctx = TraversalContext {
            max_variants: self.max_vars,
            max_missed_cleavages: self.max_cleavages,
            intervals: intervals.into(),
        };
        self.tx_pass
            .send(Arc::new(Pass::new(idx, ctx, self.done_tx.clone())))
            .context("reader gone")
    }

    /// Waits until the last graph and job of the pass is dropped.
    fn wait_traversed(&self, idx: usize) -> Result<()> {
        let done = self.supervisor.wait(&self.done_rx)?;
        ensure!(done == idx, "pass {done} finished while waiting for {idx}");
        Ok(())
    }

    /// Makes every worker send its partially filled batches.
    fn flush_workers(&self) -> Result<()> {
        let acks = self.broadcast(&self.txs_flush, |ack| ack)?;
        for ack in &acks {
            self.supervisor.wait(ack)?;
        }
        Ok(())
    }

    /// Makes every writer drain its table; collects their reports.
    fn finish_writers(&self) -> Result<PassReport> {
        let acks = self.broadcast(&self.txs_writer, WriterMsg::EndPass)?;
        let mut report = PassReport::default();
        for ack in &acks {
            report.add(self.supervisor.wait::<WriterReport>(ack)?);
        }
        Ok(report)
    }

    /// Sends every receiver a message carrying a fresh acknowledgement channel.
    fn broadcast<M, A>(
        &self,
        txs: &[Sender<M>],
        msg: impl Fn(Sender<A>) -> M,
    ) -> Result<Vec<Receiver<A>>> {
        txs.iter()
            .map(|tx| {
                let (ack_tx, ack_rx) = bounded(1);
                tx.send(msg(ack_tx))
                    .map_err(|_| anyhow!("pipeline thread gone"))?;
                Ok(ack_rx)
            })
            .collect()
    }

    /// Stops all threads and returns the first error any of them reported.
    pub fn shutdown(self) -> Result<()> {
        // closing the inputs lets every stage run dry and exit in order
        drop(self.tx_pass);
        drop(self.txs_flush);
        drop(self.txs_writer);
        self.supervisor.join()
    }
}
