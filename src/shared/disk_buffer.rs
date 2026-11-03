use anyhow::{Context, Result};

use crate::shared::{MemBuffer, Record};

pub const SIZE_LEN: usize = 4;
pub const COUNT_LEN: usize = 2;
pub const ID_LEN: usize = 10;
pub const START_LEN: usize = 2;
pub const META_LEN: usize = ID_LEN + START_LEN;
pub const DISK_HEADER_LEN: usize = SIZE_LEN + COUNT_LEN;

/// On-disk layout, one record after another:
/// size | count | (ID | START) * count | sequence
///
/// `size` counts everything after itself. The hash is not stored on disk.
#[derive(Debug, Default)]
pub struct DiskBuffer {
    data: Vec<u8>,
}

impl DiskBuffer {
    pub fn new() -> Self {
        Self::default()
    }

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

    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Clears the buffer and fills it from `reader` to EOF, reusing the allocation.
    pub fn read_from<R: std::io::Read>(&mut self, reader: &mut R) -> std::io::Result<usize> {
        self.data.clear();
        reader.read_to_end(&mut self.data)
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Encodes one record and appends it. Size checks run before any bytes
    /// are written, so a failed push leaves the buffer unchanged.
    pub fn push(&mut self, record: &Record) -> Result<()> {
        let count = u16::try_from(record.metas.len()).context("too many metas for u16 count")?;
        let body_len = COUNT_LEN + record.metas.len() * META_LEN + record.sequence.len();
        let record_size = u32::try_from(body_len).context("record too large for u32 size")?;

        self.data.reserve(SIZE_LEN + body_len);
        self.data.extend_from_slice(&record_size.to_le_bytes());
        self.data.extend_from_slice(&count.to_le_bytes());

        for meta in &record.metas {
            let mut id = [0u8; ID_LEN];
            let bytes = meta.id.as_bytes();
            let n = bytes.len().min(ID_LEN);
            id[..n].copy_from_slice(&bytes[..n]);
            self.data.extend_from_slice(&id);
            self.data.extend_from_slice(&meta.pos.to_le_bytes());
        }

        self.data.extend_from_slice(&record.sequence);
        Ok(())
    }

    /// Encodes every record in an in-memory buffer.
    pub fn from_mem(mem: &MemBuffer) -> Result<Self> {
        let mut out = Self::new();
        for record in mem.iter() {
            out.push(record)?;
        }
        Ok(out)
    }

    pub fn iter(&self) -> DiskIter<'_> {
        DiskIter {
            data: &self.data,
            offset: 0,
        }
    }
}

/// Read view of one on-disk record, including its size prefix.
#[derive(Debug, Clone, Copy)]
pub struct DiskRecord<'a> {
    bytes: &'a [u8],
}

impl<'a> DiskRecord<'a> {
    pub fn count(&self) -> usize {
        u16::from_le_bytes([self.bytes[SIZE_LEN], self.bytes[SIZE_LEN + 1]]) as usize
    }

    /// Yields (raw id bytes, pos). Ids are zero-padded to `ID_LEN`.
    pub fn metas(&self) -> impl Iterator<Item = (&'a [u8], u16)> + 'a {
        let body: &'a [u8] = self.bytes;
        let start = DISK_HEADER_LEN;
        let end = start + self.count() * META_LEN;
        body[start..end].chunks_exact(META_LEN).map(|c| {
            (
                &c[..ID_LEN],
                u16::from_le_bytes([c[ID_LEN], c[ID_LEN + 1]]),
            )
        })
    }

    pub fn sequence(&self) -> &'a [u8] {
        let body: &'a [u8] = self.bytes;
        let start = DISK_HEADER_LEN + self.count() * META_LEN;
        &body[start..]
    }
}

pub struct DiskIter<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for DiskIter<'a> {
    type Item = DiskRecord<'a>;

    fn next(&mut self) -> Option<DiskRecord<'a>> {
        let data: &'a [u8] = self.data;
        let rest = &data[self.offset..];
        if rest.len() < SIZE_LEN {
            return None;
        }
        let record_size = u32::from_le_bytes(rest[..SIZE_LEN].try_into().ok()?) as usize;
        if record_size < COUNT_LEN {
            return None;
        }
        let total = SIZE_LEN + record_size;
        if rest.len() < total {
            return None;
        }
        self.offset += total;
        Some(DiskRecord {
            bytes: &rest[..total],
        })
    }
}