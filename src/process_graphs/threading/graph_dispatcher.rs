use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::process_graphs::{graph::ProteinGraph, utilities::TraversalContext};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use tracing::info;

use crate::process_graphs::utilities::types::TraversalJob;

pub fn spawn_dispatchers(
    rx_graphs: Receiver<ProteinGraph>,
    tx_jobs: Sender<TraversalJob>,
    num_threads: usize,
    jobs_per_graph: usize,
    ctx: Arc<TraversalContext>,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::with_capacity(num_threads);

    for i in 0..num_threads {
        let rx = rx_graphs.clone();
        let tx = tx_jobs.clone();
        let ctx = Arc::clone(&ctx);

        let handle = thread::spawn(move || {
            spawn_graph_dispatcher(rx, tx, ctx, jobs_per_graph)
                .with_context(|| format!("worker {i} died"))
        });

        handles.push(handle);
    }

    Ok(handles)
}

pub fn spawn_graph_dispatcher(
    rx_graphs: Receiver<ProteinGraph>,
    tx_jobs: Sender<TraversalJob>,
    ctx: Arc<TraversalContext>,
    jobs_per_graph: usize,
) -> Result<()> {
    for graph in rx_graphs {
        let graph = Arc::new(graph);

        info!("TRAVERSAL start for {}", graph.meta_data.accessions[0]);

        let frontier = graph.build_frontier(&ctx, jobs_per_graph);

        info!(
            "FRONTIER for {}: {} jobs",
            graph.meta_data.accessions[0],
            frontier.len()
        );

        for job in frontier {
            tx_jobs.send(job)?;
        }
    }
    Ok(())
}
