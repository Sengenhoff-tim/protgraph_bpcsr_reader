use std::{
    sync::Arc, thread::{self, JoinHandle}
};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};

use crate::process_graphs::utilities::{
    TraversalContext, TraversalJob,
};
use crate::shared::EntryBuffer;

pub fn spawn_workers(
    rx_jobs: Receiver<TraversalJob>,
    rx_reuse: Receiver<EntryBuffer>,
    tx_filled: Sender<EntryBuffer>,
    batch_target_size: usize,
    num_threads: usize,
    ctx: Arc<TraversalContext>
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::with_capacity(num_threads);

    for i in 0..num_threads {
        let rx = rx_jobs.clone();
        let rx_reuse = rx_reuse.clone();
        let tx_filled = tx_filled.clone();
        let ctx = Arc::clone(&ctx);

        let handle = thread::spawn(move || {
            traversal_thread(
                rx,
                rx_reuse,
                tx_filled,
                batch_target_size,
                ctx
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
    ctx: Arc<TraversalContext>
) -> Result<()> {
    let mut batch_buffer = rx_reuse.recv()?;
    let mut qualifier_buffer = String::new();

    while let Ok(mut job) = rx.recv() {
        let graph = &job.graph;
        job.graph.tree_data.traverse_depth_first(
            &ctx,
            &mut job.path,
            &mut |path, cleaves| {
                qualifier_buffer.clear();
                graph.meta_data.build_peptide(
                    path,
                    cleaves,
                    &mut batch_buffer.data,
                    &mut qualifier_buffer,
                )?;
                flush_if_full(&mut batch_buffer, batch_target_size, &rx_reuse, &tx_filled)
            },
        )?;
    }

    if !batch_buffer.data.is_empty() {
        tx_filled.send(batch_buffer)?;
    }

    Ok(())
}

fn flush_if_full(
    batch_buffer: &mut EntryBuffer,
    batch_target_size: usize,
    rx_reuse: &Receiver<EntryBuffer>,
    tx_filled: &Sender<EntryBuffer>,
) -> Result<()> {
    if batch_buffer.data.len() >= batch_target_size {
        let mut new_buffer = rx_reuse.recv()?;
        std::mem::swap(batch_buffer, &mut new_buffer);
        tx_filled.send(new_buffer)?;
    }
    Ok(())
}