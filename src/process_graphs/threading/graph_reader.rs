use std::{
    io::BufRead,
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result};
use crossbeam_channel::Sender;
use tracing::info;

use crate::process_graphs::{
    graph::ProteinGraph,
    io::arena_io::{read_graph, read_header},
};

/// Reads converted graphs (see `arena_io`) and sends them downstream.
pub fn spawn_protein_graph_reader<R: BufRead + Send + 'static>(
    rdr: R,
    tx_protgraph: Sender<ProteinGraph>,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || {
        let mut rdr = rdr;
        let mut total_graphs = 0usize;

        read_header(&mut rdr)?;

        while let Some(pg) =
            read_graph(&mut rdr).with_context(|| format!("reading graph {total_graphs}"))?
        {
            total_graphs += 1;
            tx_protgraph.send(pg).context("Channel receiver dropped")?;
        }

        info!("INPUT read complete: {} graphs", total_graphs);
        Ok(())
    })
}
