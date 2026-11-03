use std::{
    fs::{File, create_dir_all},
    io::BufReader,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow};
use crossbeam_channel::{bounded, unbounded};

use crate::{
    parameters::Config,
    process_graphs::{
        graph::ProteinGraph,
        threading::{
            graph_dispatcher::spawn_dispatchers, spawn_protein_graph_reader, spawn_workers,
            spawn_writer_managers,
        },
        utilities::{TraversalContext, types::TraversalJob},
    },
    shared::MemBuffer,
};

const GIB: u64 = 1024 * 1024 * 1024;
const MIN_BATCH_BYTES: usize = 64 * 1024;

/// main graph processing workflow
pub fn process_graphs(config: Config) -> Result<()> {
    // config
    let cli = &config.cli;

    // create output directory
    let out_dir = &cli.outdir_path;
    create_dir_all(out_dir).with_context(|| format!("failed to create {}", out_dir.display()))?;

    // workflow is: graph reader -> dispatcher -> worker pool -> intermediate file writer

    // ---------------------------------------------------------------------------
    // set up channels
    // ---------------------------------------------------------------------------

    let channel_size = cli.avail_processors * 2;



    // graph channel: graph_reader -> dispatcher
    let (tx_graphs, rx_graphs) = bounded::<ProteinGraph>(channel_size);

    // job channel: dispatcher -> worker pool
    let (tx_jobs, rx_jobs) = bounded::<TraversalJob>(channel_size);

    // mixed output buffer pool: worker pool -> writers
    let (tx_result_buffer_empty, rx_result_buffer_empty) = bounded::<MemBuffer>(channel_size);

    let (txs_filled, rx_filled): (Vec<_>, Vec<_>) = (0..2*cli.avail_processors)
        .map(|_| unbounded::<MemBuffer>())
        .unzip();

    let needed = cli.avail_processors * cli.avail_processors + cli.avail_processors * 2;

    // Spread the memory budget (avail_memory / 10) across all buffers.
    let budget = (cli.avail_memory as u64 * GIB / 10) as usize;
    let batch_target_size = (budget / needed).max(MIN_BATCH_BYTES);

    let channel_size = needed;

    for _ in 0..channel_size {
        tx_result_buffer_empty.send(MemBuffer::with_capacity(batch_target_size))?;
    }
    // ---------------------------------------------------------------------------
    // spawn threads
    // ---------------------------------------------------------------------------

    // spawn intermediate file writer
    let bin_writer_handles = spawn_writer_managers(
        rx_filled,
        tx_result_buffer_empty,
        out_dir,
        cli.max_handles,
        cli.avail_memory,
    )?;

    let ctx = Arc::new(TraversalContext {
        max_variants: cli.max_vars,
        max_missed_cleavages: cli.max_cleavages,
        intervals: config.intervals,
    });

    let worker_handles = spawn_workers(
        rx_jobs,
        rx_result_buffer_empty,
        txs_filled,
        batch_target_size,
        cli.avail_processors,
        Arc::clone(&ctx),
    )?;

    let graph_handles = spawn_dispatchers(
        rx_graphs,
        tx_jobs,
        cli.avail_processors,
        10 * cli.avail_processors,
        ctx,
    )?;

    let graph = File::open(&cli.graph_input_path)?;
    let reader_for_graph = BufReader::new(graph);
    let reader_handle = spawn_protein_graph_reader(reader_for_graph, tx_graphs);

    // ---------------------------------------------------------------------------
    // join handles
    // ---------------------------------------------------------------------------

    reader_handle
        .join()
        .map_err(|e| anyhow!("reader thread panicked: {:?}", e))?
        .context("reader thread failed")?;

    for handle in graph_handles {
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("dispatcher panicked"))?
            .with_context(|| "dispatcher thread failed")?;
    }

    for handle in worker_handles {
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("worker panicked"))?
            .with_context(|| "worker thread failed")?;
    }

    for handle in bin_writer_handles {
        handle
            .join()
            .map_err(|e| anyhow!("bin writer thread panicked: {:?}", e))?
            .context("bin writer thread failed")?;
    }
        
    Ok(())
}
