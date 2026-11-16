//! File header and graph records.

use std::{
    fs::File,
    io::{BufRead, ErrorKind, Read, Write},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::process_graphs::graph::ProteinGraph;

const MAGIC_PREFIX: &[u8; 7] = b"PGARENA";
pub(super) const VERSION: u8 = 2;
pub const MAGIC: [u8; 8] = *b"PGARENA\x02";
pub(super) const HEADER_LEN: u64 = MAGIC.len() as u64 + 8;
pub const EXTENSION: &str = "pgarena";

/// True if the file at `path` is a converted graph file. Fails for converted files of
/// another format version.
pub fn is_arena_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut magic = [0u8; MAGIC.len()];
    match file.read_exact(&mut magic) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(false),
        Err(e) => return Err(e.into()),
    }
    if &magic[..MAGIC_PREFIX.len()] != MAGIC_PREFIX {
        return Ok(false);
    }
    ensure!(
        magic[7] == VERSION,
        "{} was converted with format version {}, expected {VERSION}; delete it to re-convert",
        path.display(),
        magic[7]
    );
    Ok(true)
}

/// Writes one graph record (without file header).
pub fn write_graph<W: Write>(
    writer: &mut W,
    graph: &ProteinGraph,
    scratch: &mut Vec<u8>,
) -> Result<()> {
    writer.write_u32::<LittleEndian>(graph.acc_id())?;
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

/// Reads the next graph record, `None` at the end of the input.
pub fn read_graph<R: BufRead>(reader: &mut R) -> Result<Option<ProteinGraph>> {
    if reader.fill_buf()?.is_empty() {
        return Ok(None);
    }

    let acc_id = reader.read_u32::<LittleEndian>()?;
    let terminal = reader.read_u32::<LittleEndian>()?;
    let n_words = usize::try_from(reader.read_u64::<LittleEndian>()?)?;

    let mut arena = vec![0u64; n_words];
    reader
        .read_u64_into::<LittleEndian>(&mut arena)
        .with_context(|| format!("reading arena of accession id {acc_id}"))?;

    ProteinGraph::from_parts(acc_id, arena.into_boxed_slice(), terminal).map(Some)
}
