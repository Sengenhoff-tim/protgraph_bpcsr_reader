use std::{
    fs::{File, create_dir_all},
    io::{BufReader, BufWriter},
    path::PathBuf,
};

use anyhow::{Context, Result, anyhow};
use crossbeam_channel::{bounded, unbounded};

use crate::parameters::Config;
use crate::{
    process_graphs::{
        graph::ProteinGraph,
        threading::{
            spawn_graph_dispatcher, spawn_protein_graph_reader, spawn_workers, spawn_writer_manager,
        },
        utilities::traversal_job::{TraversalJob, TraversalWorkerResult},
    },
    shared::BinEntry,
};

const GB: u64 = 1024 * 1024 * 1024;
const LOG_FILE_NAME: &str = "logs.csv";

/// main graph processing workflow
pub fn process_graphs(config: Config) -> Result<Vec<PathBuf>> {
    // create output directory
    let cli = &config.cli;

    let ch_graph_in_size = cli.ch_proc_in_size.unwrap_or(2);
    let ch_graph_query_size = cli.ch_proc_query_size.unwrap_or(cli.avail_processors * 2);
    let ch_bin_out_size = cli.ch_proc_out_size.unwrap_or(cli.avail_processors * 2);

    let out_dir = &cli.outdir_path;

    create_dir_all(out_dir).with_context(|| format!("failed to create {}", out_dir.display()))?;

    // set up log writer
    let logs = File::create(out_dir.join(LOG_FILE_NAME))?;
    let log_writer = BufWriter::new(logs);

    //channels
    let (tx_graphs, rx_graphs) = bounded::<ProteinGraph>(ch_graph_in_size);
    let (tx_jobs, rx_jobs) = bounded::<TraversalJob>(ch_graph_query_size);
    let (tx_entry, rx_entry) = bounded::<BinEntry>(ch_bin_out_size);
    let (tx_worker_results, rx_worker_results) = unbounded::<TraversalWorkerResult>();

    // setup graph reader
    let graph = File::open(&cli.graph_input_path)?;
    let reader_for_graph = BufReader::new(graph);

    // spawn tmp file writer
    let bin_writer_handle =
        spawn_writer_manager(out_dir, cli.hash_bits, cli.max_handles, rx_entry)?;

    let worker_handles = spawn_workers(
        rx_jobs,
        tx_entry,
        tx_worker_results,
        cli.avail_processors,
        cli.max_vars,
        (cli.avail_memory as u64 * GB * 7/10/ cli.avail_processors as u64) as usize,
    )?;

    let graph_handle = spawn_graph_dispatcher(
        rx_graphs,
        rx_worker_results,
        tx_jobs,
        config.intervals,
        log_writer,
        cli.job_splits as usize,
        cli.job_split_depth as usize,
    )?;

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

    let result = bin_writer_handle
        .join()
        .map_err(|e| anyhow!("bin writer thread panicked: {:?}", e))?
        .context("bin writer thread failed")?;

    Ok(result)
}
