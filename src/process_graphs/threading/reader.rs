use std::sync::Arc;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use tracing::info;

use crate::process_graphs::{
    io::arena_io::ArenaFile,
    utilities::{GraphMsg, Pass},
};

/// For every pass: reads all graphs of the arena file from the start and sends them
/// downstream, tagged with the pass. Runs until the pass channel closes.
pub fn reader_loop(
    arena: Arc<ArenaFile>,
    rx_pass: Receiver<Arc<Pass>>,
    tx_graphs: Sender<GraphMsg>,
) -> Result<()> {
    for pass in rx_pass {
        let mut graphs = arena.graphs()?;
        let mut n = 0usize;
        while let Some(graph) = graphs
            .next_graph()
            .with_context(|| format!("reading graph {n}"))?
        {
            n += 1;
            tx_graphs
                .send(GraphMsg {
                    graph,
                    pass: Arc::clone(&pass),
                })
                .context("graph receiver dropped")?;
        }
        info!("INTERVAL {}: read {} graphs", pass.idx, n);
    }
    Ok(())
}
