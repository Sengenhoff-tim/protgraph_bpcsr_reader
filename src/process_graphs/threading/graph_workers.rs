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
        let rx_reuse = rx_reuse.clone();
        let tx_filled = tx_filled.clone();

        let handle = thread::spawn(move || {
            traversal_thread(
                rx,
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
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    max_vars: u16,
    max_cleaves: u16,
) -> Result<()> {
    let mut batch_buffer = rx_reuse.recv()?;
    let mut qualifier_buffer = String::new();

    while let Ok(mut job) = rx.recv() {


        job.graph.traversal_data.traverse(
            &job.intervals,
            max_vars,
            max_cleaves,
            &mut job.state,
            |path, cleaves| {
                qualifier_buffer.clear();
                job.graph.meta_data.build_peptide(
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