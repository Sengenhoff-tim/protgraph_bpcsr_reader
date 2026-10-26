use std::{
    fs::{File, create_dir_all}, io::BufReader, sync::{Arc}
};

use anyhow::{Context, Result, anyhow};
use crossbeam_channel::{bounded, unbounded};

use crate::{
    parameters::Config, process_graphs::{
        graph::ProteinGraph, threading::{
            spawn_graph_dispatcher, spawn_protein_graph_reader, spawn_workers, spawn_writer_manager
        }, utilities::{
            TraversalContext, types::{TraversalJob}
        }
    }, shared::EntryBuffer
};

const GIB: u64 = 1024 * 1024 * 1024;

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

    // graph channel: graph_reader -> dispatcher

    let graph_capacity = cli.ch_proc_in_size.unwrap_or(2);
    let (tx_graphs, rx_graphs) = bounded::<ProteinGraph>(graph_capacity);

    // job channel: dispatcher -> worker pool

    let job_capacity = cli.ch_proc_query_size.unwrap_or(cli.avail_processors * 2);
    let (tx_jobs, rx_jobs) = bounded::<TraversalJob>(job_capacity);

    // ouput buffer pool: worker pool -> writer

    let ch_proc_out_size = cli.ch_proc_out_size.unwrap_or(cli.avail_processors * 2);
    let (tx_result_buffer_empty, rx_result_buffer_empty) = bounded::<EntryBuffer>(ch_proc_out_size);
    let (tx_result_buffer_full, rx_result_buffer_full) = unbounded::<EntryBuffer>();

    let batch_target_size = ((cli.avail_memory as u64 * GIB) / 10 / ch_proc_out_size as u64) as usize;

    for _ in 0..ch_proc_out_size {
        tx_result_buffer_empty.send(EntryBuffer::with_capacity(batch_target_size))?;
    }

    // ---------------------------------------------------------------------------
    // spawn threads
    // ---------------------------------------------------------------------------

    // spawn intermediate file writer
    let bin_writer_handle = spawn_writer_manager(
        rx_result_buffer_full,
        tx_result_buffer_empty,
        out_dir,
        cli.hash_bits,
        cli.max_handles,
    )?;

    let ctx = Arc::new(
        TraversalContext{
            max_variants: cli.max_vars,
            max_missed_cleavages: cli.max_cleavages,
            intervals: config.intervals
        }
    );

    let worker_handles = spawn_workers(
        rx_jobs,
        rx_result_buffer_empty,
        tx_result_buffer_full,
        batch_target_size,
        cli.avail_processors as usize,
        Arc::clone(&ctx)
    )?;

    let graph_handle = spawn_graph_dispatcher(
        rx_graphs,
        tx_jobs,
        ctx,
        10*cli.avail_processors
    );

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

    graph_handle
        .join()
        .map_err(|e| anyhow!("graph thread panicked: {:?}", e))?
        .context("graph thread failed")?;

    for handle in worker_handles {
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("worker panicked"))?
            .with_context(|| "worker thread failed")?;
    }

    bin_writer_handle
        .join()
        .map_err(|e| anyhow!("bin writer thread panicked: {:?}", e))?
        .context("bin writer thread failed")?;

    Ok(())
}
