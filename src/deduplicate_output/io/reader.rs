use std::{
    fs::File,
    io::{BufReader, ErrorKind, Read},
    path::Path,
};

use anyhow::{Context, Result};
use bincode::config::standard;

use crate::shared::BinEntry;

pub fn read_entries_binary(path: impl AsRef<Path>) -> Result<Vec<BinEntry>> {
    let path = path.as_ref();

    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;

    let mut reader = BufReader::new(file);

    let mut entries = Vec::new();

    loop {
        let mut len_buf = [0u8; 4];

        match reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("failed reading length prefix from {}", path.display())
                });
            }
        }

        let len = u32::from_le_bytes(len_buf) as usize;

        let mut bytes = vec![0u8; len];

        reader
            .read_exact(&mut bytes)
            .with_context(|| format!("failed reading {} bytes from {}", len, path.display()))?;

        let (entry, _): (BinEntry, usize) = bincode::decode_from_slice(&bytes, standard())
            .with_context(|| format!("failed to deserialize entry from {}", path.display()))?;

        entries.push(entry);
    }

    Ok(entries)
}
