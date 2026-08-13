use std::thread::{self, JoinHandle};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};

use crate::process_graphs::{
    graph::TraversalStatus, utilities::{
        TraversalState, traversal_job::{TraversalJob, TraversalWorkerResult},
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
    max_vars: u16,
    max_cleaves: u16,
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
                max_cleaves,
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
    max_vars: u16,
    max_cleaves: u16,
    limit: usize,
) -> Result<()> {
    let mut traversal_state = TraversalState::new(limit);

    let mut batch_buffer = rx_reuse.recv()?;

    let mut qualifier_buffer = String::new();

    for job in rx_jobs {
        match job.graph.traversal_data.traverse(
    job.interval,
    max_vars,
    max_cleaves,
    &mut traversal_state,
    |path, cleaves| {
        let trace: Vec<(u32, Option<u32>)> =
            path.iter().map(|s| (s.node, s.edge)).collect();

        qualifier_buffer.clear();

        job.graph.meta_data.build_peptide(
            &trace,
            cleaves,
            &mut batch_buffer.data,
            &mut qualifier_buffer,
        )?;

        if batch_buffer.data.len() >= batch_target_size {
            let mut new_buffer = rx_reuse.recv()?;
            std::mem::swap(&mut batch_buffer, &mut new_buffer);
            tx_filled.send(new_buffer)?;
        }

        Ok(())
    },
) {
    Ok(TraversalStatus::Overflow) => {
        tx_worker_results
            .send(TraversalWorkerResult::Reschedule(job))
            .with_context(|| format!("worker {worker_id} failed to reschedule"))?;
    }

    Ok(TraversalStatus::Success) => {
        tx_worker_results.send(TraversalWorkerResult::Complete)?;
    }

    Err(e) => return Err(e),
}
    }

    if !batch_buffer.data.is_empty() {
        tx_filled.send(batch_buffer)?;
    }

    Ok(())
}
