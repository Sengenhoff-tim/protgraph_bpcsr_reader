use std::{
    sync::Arc,
    thread::{self, JoinHandle}
};

use crate::process_graphs::{
    graph::{ProteinGraph, FrontierBuilder},
    utilities::Interval
};

use anyhow::{Result};
use crossbeam_channel::{Receiver, Sender};
use tracing::info;

use crate::process_graphs::utilities::{
    traversal_job::{TraversalJob},
};

pub fn spawn_graph_dispatcher(
    rx_graphs: Receiver<ProteinGraph>,
    tx_jobs: Sender<TraversalJob>,
    intervals: Arc<[Interval]>,
    max_vars: u16,
    max_cleaves: u16,
    jobs_per_graph: usize,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let mut total_graphs = 0usize;

        for graph in rx_graphs {
            let graph = Arc::new(graph);
            total_graphs += 1;

            info!("TRAVERSAL start for {}", graph.meta_data.accessions[0]);

            let frontier = FrontierBuilder::new(
                Arc::clone(&graph),
                Arc::clone(&intervals),
                max_vars,
                max_cleaves,
                jobs_per_graph,
            )
            .build();

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