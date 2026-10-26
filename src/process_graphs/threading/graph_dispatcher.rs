use std::{
    sync::Arc,
    thread::{self, JoinHandle}
};

use crate::process_graphs::{
    graph::ProteinGraph, 
    utilities::TraversalContext
};

use anyhow::{Result};
use crossbeam_channel::{Receiver, Sender};
use tracing::info;

use crate::process_graphs::utilities::{
    types::{TraversalJob},
};

pub fn spawn_graph_dispatcher(
    rx_graphs: Receiver<ProteinGraph>,
    tx_jobs: Sender<TraversalJob>,
    ctx: Arc<TraversalContext>,
    jobs_per_graph: usize,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let mut total_graphs = 0usize;

        for graph in rx_graphs {
            let graph = Arc::new(graph);
            total_graphs += 1;

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

        info!(
            "INPUT read complete: {} graphs",
            total_graphs
        );
        Ok(())
    })
}