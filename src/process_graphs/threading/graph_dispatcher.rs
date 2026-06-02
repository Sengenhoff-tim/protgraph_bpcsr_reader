use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::Arc,
    thread,
    thread::JoinHandle,
};

use anyhow::{Error, Result};
use crossbeam_channel::{Receiver, Sender};

use crate::process_graphs::utilities::traversal_job::{TraversalJob, TraversalWorkerResult};
use crate::process_graphs::{graph::ProteinGraph, utilities::Interval};

pub fn spawn_graph_dispatcher(
    rx_graphs: Receiver<ProteinGraph>,
    rx_worker_results: Receiver<TraversalWorkerResult>,
    tx_jobs: Sender<TraversalJob>,
    intervals: Vec<Interval>,
    log_writer: BufWriter<File>,
    job_splits: usize,
    job_split_depth: usize,
) -> Result<JoinHandle<Result<(), Error>>> {
    let handle = thread::spawn(move || -> Result<()> {
        let mut log_writer = log_writer;
        let mut outstanding = 0usize;

        // initial submission
        for graph in rx_graphs {
            let graph = Arc::new(graph);

            for interval in intervals.iter().cloned() {
                while let Ok(result) = rx_worker_results.try_recv() {
                    handle_worker_result(
                        result,
                        &mut outstanding,
                        &tx_jobs,
                        &mut log_writer,
                        job_splits,
                        job_split_depth,
                    )?;
                }

                tx_jobs.send(TraversalJob {
                    graph: graph.clone(),
                    interval,
                    depth: 0,
                })?;

                outstanding += 1;
            }
        }

        // drain until all recursive work completes
        while outstanding > 0 {
            let result = rx_worker_results.recv()?;

            handle_worker_result(
                result,
                &mut outstanding,
                &tx_jobs,
                &mut log_writer,
                job_splits,
                job_split_depth,
            )?;
        }
        Ok(())
    });

    Ok(handle)
}

fn handle_worker_result(
    result: TraversalWorkerResult,
    outstanding: &mut usize,
    tx_jobs: &Sender<TraversalJob>,
    log_writer: &mut BufWriter<File>,
    job_splits: usize,
    job_split_depth: usize,
) -> anyhow::Result<()> {
    match result {
        TraversalWorkerResult::Complete => {
            *outstanding -= 1;
        }

        TraversalWorkerResult::Reschedule(job) => {
            *outstanding -= 1;

            let new_depth = job.depth + 1;

            

            if new_depth > job_split_depth {
                writeln!(
                    log_writer,
                    "{},{},Incomplete traversal: split depth limit reached",
                    job.graph.meta_data.accessions[0], job.interval
                )?;

                return Ok(())
            }

            let splits = job.interval.split_to_n(job_splits);

            if splits.len() == 1 {
                writeln!(
                    log_writer,
                    "{},{},Incomplete traversal: No further splits possible",
                    job.graph.meta_data.accessions[0], job.interval
                )?;

                return Ok(());
            }
            

            for interval in &splits {
                tx_jobs.send(TraversalJob {
                    graph: job.graph.clone(),
                    interval: *interval,
                    depth: new_depth,
                })?;
            }

            *outstanding += splits.len();
        }
    }

    Ok(())
}
