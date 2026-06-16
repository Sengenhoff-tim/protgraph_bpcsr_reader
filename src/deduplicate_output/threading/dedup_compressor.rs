use std::{fs::File, io::{BufWriter, Write}, thread};
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use flate2::{Compression, write::GzEncoder};
use crate::deduplicate_output::chunk_buffer::ChunkBuffer;
use crate::deduplicate_output::io::WriterWrapper;

pub fn spawn_compressor(
    rx_chunk: Receiver<ChunkBuffer>,
    tx_chunk_empty: Sender<ChunkBuffer>,
    file: File,
    zip: bool,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let encoder = if zip {
            WriterWrapper::Compressed(GzEncoder::new(file, Compression::default()))
        } else {
            WriterWrapper::Uncompressed(file)
        };
        let mut writer = BufWriter::new(encoder);
        for mut chunk in rx_chunk {
            writer.write_all(&chunk.data)?;
            chunk.clear();
            tx_chunk_empty.send(chunk)?;
        }
        writer.flush()?;
        let inner: WriterWrapper<File> = writer.into_inner().unwrap();
        match inner {
            WriterWrapper::Compressed(enc) => { enc.finish()?; }
            WriterWrapper::Uncompressed(_) => {}
        }
        Ok(())
    })
}