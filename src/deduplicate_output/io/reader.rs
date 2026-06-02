use std::{
    fs::File,
    io::{BufReader, ErrorKind, Read},
    path::Path,
};

use anyhow::{Context, Result};
use bincode::config::standard;

use crate::shared::BinEntry;

pub fn read_entries_binary(path: impl AsRef<Path>, len_buf: &mut [u8; 4], entry_buf: &mut Vec<u8>) -> Result<Vec<BinEntry>> {
    let path = path.as_ref();

    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;

    let mut reader = BufReader::new(file);

    let mut entries: Vec<BinEntry> = Vec::new();

    loop {
        

        match reader.read(len_buf) {
            Ok(0) => break,
            Ok(4) => {}
            Ok(n) => {
                anyhow::bail!(
                    "truncated length prefix: expected 4 bytes, got {} in {}",
                    n,
                    path.display()
                );
            }       
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("failed reading length prefix from {}", path.display())
                });
            }
        }

        let len = u32::from_le_bytes(*len_buf) as usize;

        if entry_buf.len() < len {
            entry_buf.resize(len, 0);
        }

        reader
            .read_exact(&mut entry_buf[..len])
            .with_context(|| format!("failed reading {} bytes from {}", len, path.display()))?;

        let (entry, consumed): (BinEntry, usize) =
            bincode::decode_from_slice(&entry_buf[..len], standard())
                .with_context(|| format!("failed to deserialize entry from {}", path.display()))?;

        if consumed != len {
            anyhow::bail!(
                "entry contained {} trailing bytes",
                len - consumed
            );
        }

        entries.push(entry);
    }

    Ok(entries)
}
