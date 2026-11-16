//! Serialized form of converted graphs (`.pgarena`).
//!
//! ```text
//! file  = MAGIC graph*
//! graph = accession_len: u32 | accession bytes | terminal: u32 | n_words: u64 | arena words
//! ```
//! All integers little endian. The arena is stored as-is, so reading is a single bulk copy.

use std::{
    fs::File,
    io::{BufRead, ErrorKind, Read, Write},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::process_graphs::graph::ProteinGraph;

pub const MAGIC: &[u8; 8] = b"PGARENA\x01";
pub const EXTENSION: &str = "pgarena";

/// True if the file at `path` starts with the arena magic.
pub fn is_arena_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut magic = [0u8; MAGIC.len()];
    match file.read_exact(&mut magic) {
        Ok(()) => Ok(&magic == MAGIC),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn write_header<W: Write>(writer: &mut W) -> Result<()> {
    writer.write_all(MAGIC)?;
    Ok(())
}

pub fn read_header<R: Read>(reader: &mut R) -> Result<()> {
    let mut magic = [0u8; MAGIC.len()];
    reader.read_exact(&mut magic).context("reading arena header")?;
    ensure!(&magic == MAGIC, "not a graph arena file (bad magic)");
    Ok(())
}

pub fn write_graph<W: Write>(writer: &mut W, graph: &ProteinGraph, scratch: &mut Vec<u8>) -> Result<()> {
    let accession = graph.accession().as_bytes();
    writer.write_u32::<LittleEndian>(accession.len().try_into()?)?;
    writer.write_all(accession)?;
    writer.write_u32::<LittleEndian>(graph.terminal())?;

    let arena = graph.arena();
    writer.write_u64::<LittleEndian>(arena.len() as u64)?;
    scratch.clear();
    scratch.reserve(arena.len() * 8);
    for word in arena {
        scratch.extend_from_slice(&word.to_le_bytes());
    }
    writer.write_all(scratch)?;
    Ok(())
}

/// Reads the next graph, `None` at end of file.
pub fn read_graph<R: BufRead>(reader: &mut R) -> Result<Option<ProteinGraph>> {
    if reader.fill_buf()?.is_empty() {
        return Ok(None);
    }

    let acc_len = reader.read_u32::<LittleEndian>()? as usize;
    let mut accession = vec![0u8; acc_len];
    reader.read_exact(&mut accession)?;
    let accession = String::from_utf8(accession).context("accession is not utf-8")?;

    let terminal = reader.read_u32::<LittleEndian>()?;
    let n_words = usize::try_from(reader.read_u64::<LittleEndian>()?)?;

    let mut arena = vec![0u64; n_words];
    reader
        .read_u64_into::<LittleEndian>(&mut arena)
        .with_context(|| format!("reading arena of '{accession}'"))?;

    ProteinGraph::from_parts(accession, arena.into_boxed_slice(), terminal).map(Some)
}
