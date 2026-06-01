use std::thread::{self, JoinHandle};

use anyhow::{Context, Result, anyhow};
use crossbeam_channel::{Receiver, Sender};

use crate::process_graphs::{
    graph::TraversalStatus,
    utilities::{
        SubgraphForQuery,
        traversal_job::{TraversalJob, TraversalWorkerResult},
    },
};
use crate::shared::BinEntry;

pub fn spawn_workers(
    rx_jobs: Receiver<TraversalJob>,
    tx_entry: Sender<BinEntry>,
    tx_worker_results: Sender<TraversalWorkerResult>,
    num_threads: usize,
    max_vars: u8,
    limit: usize,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::with_capacity(num_threads);

    for i in 0..num_threads {
        let rx_jobs = rx_jobs.clone();
        let tx_entry = tx_entry.clone();
        let tx_worker_results = tx_worker_results.clone();

        let handle: JoinHandle<Result<()>> = thread::spawn(move || {
            traversal_thread(i, rx_jobs, tx_entry, tx_worker_results, max_vars, limit)
                .with_context(|| format!("worker {i} died"))
        });

        handles.push(handle);
    }

    Ok(handles)
}

fn traversal_thread(
    worker_id: usize,
    rx_jobs: Receiver<TraversalJob>,
    tx_entry: Sender<BinEntry>,
    tx_worker_results: Sender<TraversalWorkerResult>,
    max_vars: u8,
    limit: usize,
) -> Result<()> {
    let mut traversal_state = SubgraphForQuery::new(limit)?;

    for job in rx_jobs {
        match job
            .graph
            .traversal_data
            .traverse(job.interval, max_vars, &mut traversal_state)
        {
            Ok(TraversalStatus::Overflow) => {
                tx_worker_results
                    .send(TraversalWorkerResult::Reschedule(job))
                    .with_context(|| format!("worker {worker_id} failed to reschedule"))?;
            }

            Ok(TraversalStatus::Success) => {
                let mut cur =
                    traversal_state.head_at_node[job.graph.traversal_data.nodes.len() - 1];

                loop {
                    let prev =
                        unsafe { traversal_state.arena.get_unchecked(cur as usize).previous };

                    let trace = traversal_state.reconstruct_trace(cur);

                    if let Ok(Some(entry)) = job.graph.meta_data.build_peptide(&trace) {
                        let _ = tx_entry.send(entry);
                    }

                    if prev == 0 {
                        break;
                    }

                    cur = prev as usize;
                }

                tx_worker_results.send(TraversalWorkerResult::Complete)?;
            }

            Err(e) => {
                return Err(anyhow!("worker {worker_id} traverse error: {e:?}"));
            }
        }
    }
    Ok(())
}
