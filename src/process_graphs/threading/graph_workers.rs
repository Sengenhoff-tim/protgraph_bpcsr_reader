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
use crate::shared::EntryBuffer;

pub fn spawn_workers(
    rx_jobs: Receiver<TraversalJob>,
    tx_worker_results: Sender<TraversalWorkerResult>,
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    num_threads: usize,
    max_vars: u8,
    limit: usize,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::with_capacity(num_threads);

    for i in 0..num_threads {
        let rx_jobs = rx_jobs.clone();
        let tx_worker_results = tx_worker_results.clone();
        let rx_reuse = rx_reuse.clone();
        let tx_filled = tx_filled.clone();

        let handle: JoinHandle<Result<()>> = thread::spawn(move || {
            traversal_thread(
                i,
                rx_jobs,
                tx_worker_results,
                rx_reuse,
                tx_filled,
                batch_target_size,
                max_vars,
                limit,
            )
            .with_context(|| format!("worker {i} died"))
        });

        handles.push(handle);
    }

    Ok(handles)
}

fn traversal_thread(
    worker_id: usize,
    rx_jobs: Receiver<TraversalJob>,
    tx_worker_results: Sender<TraversalWorkerResult>,
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    max_vars: u8,
    limit: usize,
) -> Result<()> {
    let mut traversal_state = SubgraphForQuery::new(limit);

    let mut batch_buffer = rx_reuse.recv()?;

    let mut qualifier_buffer = String::new();

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
                let final_states =
                    &traversal_state.states_at_node[job.graph.traversal_data.nodes.len() - 1];

                for &state_id in final_states {
                    let trace = traversal_state.reconstruct_trace(state_id);

                    qualifier_buffer.clear();

                    job.graph.meta_data.build_peptide(
                        &trace,
                        &mut batch_buffer.data,
                        &mut qualifier_buffer,
                    )?;

                    if batch_buffer.data.len() >= batch_target_size {
                        tx_filled.send(batch_buffer)?;
                        batch_buffer = rx_reuse.recv()?;
                    }
                }

                tx_worker_results.send(TraversalWorkerResult::Complete)?;
            }

            Err(e) => {
                return Err(anyhow!("worker {worker_id} traverse error: {e:?}"));
            }
        }
    }

    if !batch_buffer.data.is_empty() {
        tx_filled.send(batch_buffer)?;
    }

    Ok(())
}
