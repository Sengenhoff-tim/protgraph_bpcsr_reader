//! Shard file records, one after another:
//!
//! ```text
//! seq_len: u32 | n_acc: u32 | packed sequence | n_acc accession ids (w bytes each)
//! ```
//! All integers little endian. The id width `w` is fixed per run and not stored.

use std::io::Write;

use anyhow::{Context, Result, ensure};

const HEADER_LEN: usize = 8;

/// Writes one shard record to `out`. `accs` holds whole `w`-byte ids.
pub fn write_record<W: Write>(out: &mut W, seq: &[u8], accs: &[u8], w: usize) -> Result<()> {
    ensure!(
        accs.len().is_multiple_of(w),
        "accession bytes not a multiple of {w}"
    );
    let seq_len = u32::try_from(seq.len()).context("sequence too long")?;
    let n_acc = u32::try_from(accs.len() / w).context("too many accessions")?;
    out.write_all(&seq_len.to_le_bytes())?;
    out.write_all(&n_acc.to_le_bytes())?;
    out.write_all(seq)?;
    out.write_all(accs)?;
    Ok(())
}

#[derive(Debug, Default)]
pub struct DiskBuffer {
    data: Vec<u8>,
}

/// One shard record: a sequence and its accession id bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskRecord<'a> {
    pub seq: &'a [u8],
    pub accs: &'a [u8],
}

impl DiskBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: Vec::with_capacity(cap),
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Clears the buffer and fills it from `reader` to EOF, reusing the allocation.
    pub fn read_from<R: std::io::Read>(&mut self, reader: &mut R) -> std::io::Result<usize> {
        self.data.clear();
        reader.read_to_end(&mut self.data)
    }

    /// Appends one record. `accs` holds whole `w`-byte ids.
    pub fn push(&mut self, seq: &[u8], accs: &[u8], w: usize) -> Result<()> {
        // on error nothing is written: the checks run before the first byte
        write_record(&mut self.data, seq, accs, w)
    }

    /// Iterates the records; a truncated trailing record ends the iteration with an error.
    pub fn iter(&self, w: usize) -> impl Iterator<Item = Result<DiskRecord<'_>>> {
        let data = &self.data[..];
        let mut offset = 0usize;
        std::iter::from_fn(move || {
            if offset == data.len() {
                return None;
            }
            let rest = &data[offset..];
            if rest.len() < HEADER_LEN {
                offset = data.len();
                return Some(Err(anyhow::anyhow!("truncated shard record header")));
            }
            let seq_len = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
            let n_acc = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
            let total = HEADER_LEN + seq_len + n_acc * w;
            if rest.len() < total {
                offset = data.len();
                return Some(Err(anyhow::anyhow!("truncated shard record")));
            }
            offset += total;
            Some(Ok(DiskRecord {
                seq: &rest[HEADER_LEN..HEADER_LEN + seq_len],
                accs: &rest[HEADER_LEN + seq_len..total],
            }))
        })
    }
}
