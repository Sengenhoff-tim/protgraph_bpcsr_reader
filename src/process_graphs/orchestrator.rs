use std::{
    fs::{File, create_dir_all},
    io::{BufReader, BufWriter},
    path::PathBuf,
    sync::Arc,
    thread,
};

use anyhow::{Context, Result, anyhow};
use crossbeam_channel::bounded;

use crate::process_graphs::{
    graph::ProteinGraph,
    threading::{spawn_graph_dispatcher, spawn_protein_graph_reader, spawn_writer_manager},
};
use crate::{parameters::Config, process_graphs::threading::graph_workers::WorkerArgs};

const GB: u64 = 1024 * 1024 * 1024;
const LOG_FILE_NAME: &str = "logs.csv";

/// main graph processing workflow
pub fn process_graphs(config: Config) -> Result<Vec<PathBuf>> {
    // create output directory
    let cli = &config.cli;

    let ch_graph_in_size = cli.ch_proc_in_size.unwrap_or(2);
    let ch_bin_out_size = cli.ch_proc_out_size.unwrap_or(cli.avail_processors * 2);

    let out_dir = &cli.outdir_path;

    create_dir_all(out_dir).with_context(|| format!("failed to create {}", out_dir.display()))?;

    // set up log writer
    let logs = File::create(out_dir.join(LOG_FILE_NAME))?;
    let log_writer = BufWriter::new(logs);

    // setup graph reader
    let graph = File::open(&cli.graph_input_path)?;
    let reader_for_graph = BufReader::new(graph);
    let (tx_graph, rx_graph) = bounded::<Result<ProteinGraph>>(ch_graph_in_size);
    let reader_handle = thread::spawn(|| spawn_protein_graph_reader(reader_for_graph, tx_graph));

    // spawn tmp file writer
    let (tx_entry, bin_writer_handle) =
        spawn_writer_manager(out_dir, cli.hash_bits, cli.max_handles, ch_bin_out_size)?;

    //process graphs
    let intervals = Arc::new(config.intervals);

    let worker_args = WorkerArgs {
        max_vars: cli.max_vars,
        limit: (cli.avail_memory as u64 * GB) as usize,
        n_splits: cli.job_splits,
        max_depth: cli.job_split_depth,
    };

    let graph_handle = spawn_graph_dispatcher(
        rx_graph,
        tx_entry,
        intervals,
        cli.avail_processors,
        log_writer,
        worker_args,
    )?;

    graph_handle
        .join()
        .map_err(|e| anyhow!("thread panicked: {:?}", e))??;

    reader_handle.join().unwrap();

    let result = bin_writer_handle.join().unwrap()?;

    Ok(result)
}
