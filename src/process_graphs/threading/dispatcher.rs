use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use tracing::debug;

use crate::process_graphs::utilities::{GraphMsg, TraversalJob};

/// Splits every graph into frontier jobs for the workers.
pub fn dispatcher_loop(
    rx_graphs: Receiver<GraphMsg>,
    tx_jobs: Sender<TraversalJob>,
    jobs_per_graph: usize,
) -> Result<()> {
    for GraphMsg { graph, pass } in rx_graphs {
        let graph = Arc::new(graph);
        let frontier = graph.build_frontier(&pass.ctx, jobs_per_graph);
        debug!(
            "FRONTIER for accession {} (interval {}): {} jobs",
            graph.acc_id(),
            pass.idx,
            frontier.len()
        );
        for path in frontier {
            tx_jobs.send(TraversalJob {
                graph: Arc::clone(&graph),
                path,
                pass: Arc::clone(&pass),
            })?;
        }
    }
    Ok(())
}
