use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Take},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use byteorder::{LittleEndian, ReadBytesExt};

use super::{
    MAGIC,
    arena_format::{HEADER_LEN, VERSION},
    read_graph,
};
use crate::process_graphs::graph::ProteinGraph;

/// An opened arena file: accession table in memory, graphs streamed per pass.
#[derive(Debug, Clone)]
pub struct ArenaFile {
    path: PathBuf,
    table_offset: u64,
    accessions: Vec<String>,
}

impl ArenaFile {
    pub fn open(path: &Path) -> Result<Self> {
        let mut reader = BufReader::new(
            File::open(path).with_context(|| format!("opening {}", path.display()))?,
        );
        let mut magic = [0u8; MAGIC.len()];
        reader
            .read_exact(&mut magic)
            .context("reading arena header")?;
        if magic != MAGIC {
            bail!(
                "{} is not a graph arena file of version {VERSION}",
                path.display()
            );
        }
        let table_offset = reader.read_u64::<LittleEndian>()?;
        ensure!(
            table_offset >= HEADER_LEN,
            "arena file was not finished (no accession table)"
        );

        reader.seek(SeekFrom::Start(table_offset))?;
        let n = reader
            .read_u32::<LittleEndian>()
            .context("reading accession table")? as usize;
        let mut accessions = Vec::with_capacity(n);
        for _ in 0..n {
            let len = reader.read_u32::<LittleEndian>()? as usize;
            let mut bytes = vec![0u8; len];
            reader.read_exact(&mut bytes)?;
            accessions.push(String::from_utf8(bytes).context("accession is not utf-8")?);
        }

        Ok(Self {
            path: path.to_path_buf(),
            table_offset,
            accessions,
        })
    }

    pub fn accessions(&self) -> &[String] {
        &self.accessions
    }

    /// A fresh reader over all graphs, from the first.
    pub fn graphs(&self) -> Result<GraphReader> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(HEADER_LEN))?;
        Ok(GraphReader {
            reader: BufReader::new(file.take(self.table_offset - HEADER_LEN)),
        })
    }
}

pub struct GraphReader {
    reader: BufReader<Take<File>>,
}

impl GraphReader {
    pub fn next_graph(&mut self) -> Result<Option<ProteinGraph>> {
        read_graph(&mut self.reader)
    }
}
