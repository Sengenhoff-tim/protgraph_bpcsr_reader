use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{Context, Result};
use byteorder::{LittleEndian, WriteBytesExt};

use super::{MAGIC, write_graph};
use crate::process_graphs::graph::ProteinGraph;

/// Streams graphs into a new arena file; the accession table is appended by [`finish`](Self::finish).
pub struct ArenaWriter {
    writer: BufWriter<File>,
    scratch: Vec<u8>,
}

impl ArenaWriter {
    pub fn create(path: &Path) -> Result<Self> {
        let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&MAGIC)?;
        writer.write_u64::<LittleEndian>(0)?; // table offset, patched in finish
        Ok(Self {
            writer,
            scratch: Vec::new(),
        })
    }

    pub fn write_graph(&mut self, graph: &ProteinGraph) -> Result<()> {
        write_graph(&mut self.writer, graph, &mut self.scratch)
    }

    pub fn finish(mut self, accessions: &[String]) -> Result<()> {
        let table_offset = self.writer.stream_position()?;
        self.writer.write_u32::<LittleEndian>(
            accessions.len().try_into().context("too many accessions")?,
        )?;
        for acc in accessions {
            self.writer
                .write_u32::<LittleEndian>(acc.len().try_into()?)?;
            self.writer.write_all(acc.as_bytes())?;
        }
        let mut file = self.writer.into_inner().map_err(|e| e.into_error())?;
        file.seek(SeekFrom::Start(MAGIC.len() as u64))?;
        file.write_u64::<LittleEndian>(table_offset)?;
        file.sync_data().ok();
        Ok(())
    }
}
