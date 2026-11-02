use std::{io::Write, thread};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crate::deduplicate_output::{
    chunk_buffer::ChunkBuffer,
    io::{write_meta, write_sequences},
    threading::dedup_workers::WorkerResult,
};
use crate::shared::EntryBuffer;

const CHUNK_SIZE: usize = 256 * 1024;

pub fn spawn_writers(
    rx_entry: Receiver<WorkerResult>,
    tx_buffer_empty: Sender<EntryBuffer>,
    tx_seq_chunk: Sender<ChunkBuffer>,
    tx_meta_chunk: Sender<ChunkBuffer>,
    rx_seq_empty: Receiver<ChunkBuffer>,
    rx_meta_empty: Receiver<ChunkBuffer>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let mut seq_chunk = rx_seq_empty.recv()?;
        let mut meta_chunk = rx_meta_empty.recv()?;

        let mut id: u128 = 0;
        let mut seq_scratch = Vec::new();

        for result in rx_entry {
            let WorkerResult { mut buffer, groups } = result;

            for (seq, metas) in groups {
                let id_start = id;

                for meta in &metas {
                    id += 1;
                    write_meta(&mut meta_chunk.data, meta.get(&buffer))?;
                }

                write_sequences(
                    &mut seq_chunk.data,
                    id_start,
                    id - id_start,
                    seq.get(&buffer),
                    &mut seq_scratch,
                )?;

                if seq_chunk.data.len() >= CHUNK_SIZE {
                    tx_seq_chunk.send(seq_chunk)?;
                    seq_chunk = rx_seq_empty.recv()?;
                }
                if meta_chunk.data.len() >= CHUNK_SIZE {
                    tx_meta_chunk.send(meta_chunk)?;
                    meta_chunk = rx_meta_empty.recv()?;
                }
            }

            buffer.clear();
            let _ = tx_buffer_empty.send(buffer);
        }

        if !seq_chunk.data.is_empty() {
            tx_seq_chunk.send(seq_chunk)?;
        }
        if !meta_chunk.data.is_empty() {
            tx_meta_chunk.send(meta_chunk)?;
        }

        drop(tx_seq_chunk);
        drop(tx_meta_chunk);

        Ok(())
    })
}