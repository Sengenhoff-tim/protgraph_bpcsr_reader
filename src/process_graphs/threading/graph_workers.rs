use std::{
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Arc,
    },
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};

use crate::process_graphs::utilities::{
    traversal_job::{Msg, TraversalJob},
    PathState, TraversalState,
};
use crate::shared::EntryBuffer;

pub fn spawn_workers(
    rx_jobs: Receiver<Msg>,
    tx_jobs: Sender<Msg>,
    pending: Arc<AtomicUsize>,
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    num_threads: usize,
    max_vars: u16,
    max_cleaves: u16,
    split_depth: usize,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::with_capacity(num_threads);

    for i in 0..num_threads {
        let rx = rx_jobs.clone();
        let tx = tx_jobs.clone();
        let pending = pending.clone();
        let rx_reuse = rx_reuse.clone();
        let tx_filled = tx_filled.clone();

        let handle = thread::spawn(move || {
            traversal_thread(
                rx,
                tx,
                pending,
                num_threads,
                rx_reuse,
                tx_filled,
                batch_target_size,
                max_vars,
                max_cleaves,
                split_depth,
            )
            .with_context(|| format!("worker {i} died"))
        });

        handles.push(handle);
    }

    Ok(handles)
}

fn traversal_thread(
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    pending: Arc<AtomicUsize>,
    num_workers: usize,
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    max_vars: u16,
    max_cleaves: u16,
    split_depth: usize,
) -> Result<()> {
    let mut batch_buffer = rx_reuse.recv()?;
    let mut qualifier_buffer = String::new();

    while let Ok(msg) = rx.recv() {
        let mut job = match msg {
            Msg::Job(j) => j,
            Msg::Stop => break,
        };

        let graph = job.graph.clone();
        let intervals = job.intervals.clone();

        let mut offload = |path: &[PathState]| -> bool {
            let child = TraversalJob {
                graph: graph.clone(),
                intervals: intervals.clone(),
                state: TraversalState::from_path(path.to_vec()),
            };
            pending.fetch_add(1, SeqCst);
            match tx.try_send(Msg::Job(child)) {
                Ok(()) => true,
                Err(_) => {
                    pending.fetch_sub(1, SeqCst);
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
            split_depth,
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

        if pending.fetch_sub(1, SeqCst) == 1 {
            for _ in 0..num_workers {
                tx.send(Msg::Stop)?;
            }
        }
    }

    if !batch_buffer.data.is_empty() {
        tx_filled.send(batch_buffer)?;
    }
    Ok(())
}