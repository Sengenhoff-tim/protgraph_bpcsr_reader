use std::{
    fs::{File, create_dir_all}, io::BufReader, sync::{Arc}
};

use anyhow::{Context, Result, anyhow};
use crossbeam_channel::{bounded, unbounded};

use crate::{parameters::Config, process_graphs::utilities::{Interval, traversal_job::{TraversalJob}}};
use crate::{
    process_graphs::{
        graph::ProteinGraph,
        threading::{
            spawn_graph_dispatcher, spawn_protein_graph_reader, spawn_workers, spawn_writer_manager,
        },
        //utilities::traversal_job::{TraversalJob},
    },
    shared::EntryBuffer,
};

const GIB: u64 = 1024 * 1024 * 1024;

/// main graph processing workflow
pub fn process_graphs(config: Config) -> Result<()> {
    // create output directory
    let cli = &config.cli;

    let ch_graph_in_size = cli.ch_proc_in_size.unwrap_or(2);
    let ch_graph_query_size = cli.ch_proc_query_size.unwrap_or(cli.avail_processors * 2);
    let ch_proc_out_size = cli.ch_proc_out_size.unwrap_or(cli.avail_processors * 2);

    let out_dir = &cli.outdir_path;

    create_dir_all(out_dir).with_context(|| format!("failed to create {}", out_dir.display()))?;

    //channels
    let (tx_graphs, rx_graphs) = bounded::<ProteinGraph>(ch_graph_in_size);

    let (tx_jobs, rx_jobs) = bounded::<TraversalJob>(ch_graph_query_size);
    //let (tx_worker_results, rx_worker_results) = unbounded::<Msg>();

    let (tx_result_buffer_empty, rx_result_buffer_empty) = bounded::<EntryBuffer>(ch_proc_out_size);
    let (tx_result_buffer_full, rx_result_buffer_full) = unbounded::<EntryBuffer>();

    // setup graph reader
    let graph = File::open(&cli.graph_input_path)?;
    let reader_for_graph = BufReader::new(graph);

    let batch_target_size =
        ((cli.avail_memory as u64 * GIB) / 10 / ch_proc_out_size as u64) as usize;

    for _ in 0..ch_proc_out_size {
        tx_result_buffer_empty.send(EntryBuffer::with_capacity(batch_target_size))?;
    }

    // spawn tmp file writer
    let bin_writer_handle = spawn_writer_manager(
        rx_result_buffer_full,
        tx_result_buffer_empty,
        out_dir,
        cli.hash_bits,
        cli.max_handles,
    )?;

    let intervals: Arc<[Interval]> = config.intervals.into();

    let worker_handles = spawn_workers(
        rx_jobs,
        tx_jobs.clone(),
        rx_result_buffer_empty,
        tx_result_buffer_full,
        batch_target_size,
        cli.avail_processors as usize,
        cli.max_vars,
        cli.max_cleavages,
    )?;

    let graph_handle = spawn_graph_dispatcher(
        rx_graphs,
        tx_jobs,
        intervals,
    );

    let reader_handle = spawn_protein_graph_reader(reader_for_graph, tx_graphs);

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
