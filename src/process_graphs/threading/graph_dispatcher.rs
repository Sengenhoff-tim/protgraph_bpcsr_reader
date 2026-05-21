type DispatcherHandle = JoinHandle<anyhow::Result<BufWriter<File>>>;

use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::{Arc, atomic::AtomicBool},
    thread,
    thread::JoinHandle,
};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};

use crate::process_graphs::threading::graph_workers::{WorkerArgs, spawn_workers};
use crate::process_graphs::{graph::ProteinGraph, utilities::Interval};
use crate::shared::BinEntry;

pub fn spawn_graph_dispatcher(
    protein_graphs: Receiver<Result<ProteinGraph>>,
    tx_entry: Sender<BinEntry>,
    intervals: Arc<Vec<Interval>>,
    t_count: usize,
    log_writer: BufWriter<File>,
    worker_args: WorkerArgs,
) -> Result<DispatcherHandle> {
    let handle = thread::spawn(move || -> anyhow::Result<BufWriter<File>> {
        let mut log_writer = log_writer;
        let args = Arc::new(worker_args);

        for graph in protein_graphs {
            let protein_graph = graph?;

            // for logging
            let accession = protein_graph.meta_data.accessions[0].clone();
            let incomplete = Arc::new(AtomicBool::new(false));

            spawn_workers(
                protein_graph,
                intervals.clone(),
                tx_entry.clone(),
                t_count,
                Arc::clone(&incomplete),
                Arc::clone(&args),
            )?;

            if incomplete.load(std::sync::atomic::Ordering::Relaxed) {
                writeln!(
                    log_writer,
                    "{},Incomplete traversal due to memory limit",
                    accession
                )?;
            }
        }

        log_writer.flush()?;
        Ok(log_writer)
    });

    Ok(handle)
}
