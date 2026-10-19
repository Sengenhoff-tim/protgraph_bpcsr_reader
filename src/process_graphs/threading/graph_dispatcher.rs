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
    tx_jobs: Sender<TraversalJob>,
    intervals: Arc<[Interval]>,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let mut total = 0usize;

        for graph in rx_graphs {
            let graph = Arc::new(graph);
            total += 1;

            info!("START, Traversal for {}", graph.meta_data.accessions[0]);

            tx_jobs.send(TraversalJob {
                graph,
                intervals: intervals.clone(),
                state: TraversalState::new(),
            })?;
        }

        info!("FINISHED, Processed {} graphs", total);
        Ok(())
    })
}