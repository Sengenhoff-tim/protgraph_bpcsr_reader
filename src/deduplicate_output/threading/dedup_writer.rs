use std::{io::Write, thread};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crate::deduplicate_output::{
    chunk_buffer::ChunkBuffer,
    io::{write_meta, write_sequences},
    threading::dedup_workers::WorkerResult,
};
use crate::shared::EntryBuffer;

const META_HEADER_LINE: &str = "ID,ACC,STARTPOS,ENDPOS,MISSCLEAVAGE,QUALIFIERS";
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

        writeln!(meta_chunk.data, "{}", META_HEADER_LINE)?;

        let mut id: u128 = 0;

        for mut result in rx_entry {
            let buffer = &mut result.buffer;
            for (seq, metas) in result.groups {
                id += 1;
                write_sequences(&mut seq_chunk.data, id, seq.get(buffer))?;
                for meta in metas {
                    write_meta(&mut meta_chunk.data, id, meta.get(buffer))?;
                }
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
            if tx_buffer_empty.send(result.buffer).is_err() {
                continue;
            }
        }

        // flush remaining
        if !seq_chunk.data.is_empty() {
            tx_seq_chunk.send(seq_chunk)?;
        }
        if !meta_chunk.data.is_empty() {
            tx_meta_chunk.send(meta_chunk)?;
        }

        // dropping tx_seq_chunk and tx_meta_chunk here signals compressors to finish
        drop(tx_seq_chunk);
        drop(tx_meta_chunk);

        Ok(())
    })
}