use std::{
    thread::{self, JoinHandle}
};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};

use crate::process_graphs::utilities::{
    traversal_job::{TraversalJob},
    PathState, TraversalState,
};
use crate::shared::EntryBuffer;

pub fn spawn_workers(
    rx_jobs: Receiver<TraversalJob>,
    tx_jobs: Sender<TraversalJob>,
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    num_threads: usize,
    max_vars: u16,
    max_cleaves: u16,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::with_capacity(num_threads);

    for i in 0..num_threads {
        let rx = rx_jobs.clone();
        let tx = tx_jobs.clone();
        let rx_reuse = rx_reuse.clone();
        let tx_filled = tx_filled.clone();

        let handle = thread::spawn(move || {
            traversal_thread(
                rx,
                tx,
                rx_reuse,
                tx_filled,
                batch_target_size,
                max_vars,
                max_cleaves
            )
            .with_context(|| format!("worker {i} died"))
        });

        handles.push(handle);
    }

    Ok(handles)
}

fn traversal_thread(
    rx: Receiver<TraversalJob>,
    tx: Sender<TraversalJob>,
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    max_vars: u16,
    max_cleaves: u16,
) -> Result<()> {
    let mut batch_buffer = rx_reuse.recv()?;
    let mut qualifier_buffer = String::new();

    while let Ok(mut job) = rx.recv() {

        let graph = job.graph.clone();
        let intervals = job.intervals.clone();

        let mut offload = |path: &[PathState]| -> bool {
            let child = TraversalJob {
                graph: graph.clone(),
                intervals: intervals.clone(),
                state: TraversalState::from_path(path.to_vec()),
            };
            match tx.try_send(child) {
                Ok(()) => true,
                Err(_) => {
                    false
                }
            }
        };

        graph.traversal_data.traverse(
            &intervals,
            max_vars,
            max_cleaves,
            &mut job.state,
            &mut offload,
            |path, cleaves| {
                qualifier_buffer.clear();
                graph.meta_data.build_peptide(
                    path,
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
        )?;
    }

    if !batch_buffer.data.is_empty() {
        tx_filled.send(batch_buffer)?;
    }
    Ok(())
}