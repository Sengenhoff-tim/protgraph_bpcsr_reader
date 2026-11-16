//! An output file written by a compressor thread in chunks.

use std::{fs::File, thread::JoinHandle};

use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender, bounded};

use super::{chunk_buffer::ChunkBuffer, threading::spawn_compressor};

const CHUNK_SIZE: usize = 256 * 1024;
const CHUNK_POOL_SIZE: usize = 8;

/// Bytes are collected in a chunk; full chunks go to the compressor thread, which
/// returns them empty through a pool.
pub struct CompressedStream {
    chunk: ChunkBuffer,
    tx_full: Sender<ChunkBuffer>,
    rx_empty: Receiver<ChunkBuffer>,
    handle: JoinHandle<Result<()>>,
}

impl CompressedStream {
    pub fn open(file: File, zip: bool) -> Result<Self> {
        let (tx_full, rx_full) = bounded::<ChunkBuffer>(CHUNK_POOL_SIZE);
        let (tx_empty, rx_empty) = bounded::<ChunkBuffer>(CHUNK_POOL_SIZE);
        for _ in 0..CHUNK_POOL_SIZE {
            tx_empty.send(ChunkBuffer::with_capacity(CHUNK_SIZE))?;
        }
        let handle = spawn_compressor(rx_full, tx_empty, file, zip);
        Ok(Self {
            chunk: rx_empty.recv()?,
            tx_full,
            rx_empty,
            handle,
        })
    }

    /// The current chunk; call [`Self::rotate_if_full`] after writing to it.
    pub fn buf(&mut self) -> &mut Vec<u8> {
        &mut self.chunk.data
    }

    pub fn rotate_if_full(&mut self) -> Result<()> {
        if self.chunk.data.len() >= CHUNK_SIZE {
            let empty = self
                .rx_empty
                .recv()
                .map_err(|_| anyhow!("compressor stopped"))?;
            let full = std::mem::replace(&mut self.chunk, empty);
            self.tx_full
                .send(full)
                .map_err(|_| anyhow!("compressor stopped"))?;
        }
        Ok(())
    }

    /// Writes the last chunk and waits for the compressor to finish the file.
    pub fn finish(self) -> Result<()> {
        self.tx_full
            .send(self.chunk)
            .map_err(|_| anyhow!("compressor stopped"))?;
        drop(self.tx_full);
        self.handle
            .join()
            .map_err(|_| anyhow!("compressor panicked"))?
    }
}
