use std::{
    sync::Arc,
    thread,
    thread::JoinHandle,
};

use anyhow::{Error, Result};
use crossbeam_channel::{Receiver, Sender};
use tracing::{info, warn};

use crate::process_graphs::utilities::traversal_job::{TraversalJob, TraversalWorkerResult};
use crate::process_graphs::{graph::ProteinGraph, utilities::Interval};

pub fn spawn_graph_dispatcher(
    rx_graphs: Receiver<ProteinGraph>,
    rx_worker_results: Receiver<TraversalWorkerResult>,
    tx_jobs: Sender<TraversalJob>,
    intervals: Box<[Interval]>,
    job_splits: usize,
    job_split_depth: usize,
) -> Result<JoinHandle<Result<(), Error>>> {
    let handle = thread::spawn(move || -> Result<()> {

        let mut total = 0;

        // initial submission
        for graph in rx_graphs {
            let graph = Arc::new(graph);
            total += 1;

            tx_jobs.send(TraversalJob {
                graph: graph.clone(),
                intervals: intervals.clone(),
                depth: 0,
            })?;
        }
        info!("FINISHED,Processed {} graphs", total);
        Ok(())
    });

    Ok(handle)
}
/* 
fn handle_worker_result(
    result: TraversalWorkerResult,
    outstanding: &mut usize,
    tx_jobs: &Sender<TraversalJob>,
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
                warn!(
                    "{},Incomplete traversal: split depth limit reached,{}",
                    job.graph.meta_data.accessions[0], job.intervals
                );
                return Ok(());
            }

            let splits = job.interval.split_to_n(job_splits);

            if splits.len() == 1 {
                warn!(
                    "{},Incomplete traversal: No further splits possible,{}",
                    job.graph.meta_data.accessions[0], job.interval
                );

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
    */
