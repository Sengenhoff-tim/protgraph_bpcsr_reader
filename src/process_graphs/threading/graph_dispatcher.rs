use std::{
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Arc,
    },
    thread::{self, JoinHandle},
};

use anyhow::{Result};
use crossbeam_channel::{Receiver, Sender};
use tracing::info;

use crate::process_graphs::graph::ProteinGraph;
use crate::process_graphs::utilities::{
    traversal_job::{Msg, TraversalJob},
    Interval, TraversalState,
};

pub fn spawn_graph_dispatcher(
    rx_graphs: Receiver<ProteinGraph>,
    tx_jobs: Sender<Msg>,
    pending: Arc<AtomicUsize>,
    intervals: Arc<[Interval]>,
    num_workers: usize,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let mut total = 0usize;

        for graph in rx_graphs {
            let graph = Arc::new(graph);
            total += 1;

            // Count the root job before sending so the total never hits zero early.
            pending.fetch_add(1, SeqCst);
            info!("START, Traversal for {}", graph.meta_data.accessions[0]);

            tx_jobs.send(Msg::Job(TraversalJob {
                graph,
                intervals: intervals.clone(),
                state: TraversalState::new(),
            }))?;
        }

        // Release the dispatcher's own hold. If no jobs are in flight, shut the workers down.
        if pending.fetch_sub(1, SeqCst) == 1 {
            for _ in 0..num_workers {
                tx_jobs.send(Msg::Stop)?;
            }
        }

        info!("FINISHED, Processed {} graphs", total);
        Ok(())
    })
}