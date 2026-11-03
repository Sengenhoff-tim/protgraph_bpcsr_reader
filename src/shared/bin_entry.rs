use anyhow::Result;

pub const SIZE_LEN: usize = 4;
pub const COUNT_LEN: usize = 2;
pub const ID_LEN: usize = 10;
pub const START_LEN: usize = 2;
pub const HASH_LEN: usize = 8;
pub const META_LEN: usize = ID_LEN + START_LEN;

/// In-memory header: size | count | hash
pub const MEM_HEADER_LEN: usize = SIZE_LEN + COUNT_LEN + HASH_LEN;
/// On-disk header: size | count
pub const DISK_HEADER_LEN: usize = SIZE_LEN + COUNT_LEN;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SeqRef { pub start: u64, pub len: u32 }

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct MetaRef { pub start: u64, pub len: u32 }

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct EntryRef { pub start: u64, pub len: u32 }

impl SeqRef {
    #[inline]
    pub fn get<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let s = self.start as usize;
        &data[s..s + self.len as usize]
    }
}

impl MetaRef {
    #[inline]
    pub fn get<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let s = self.start as usize;
        &data[s..s + self.len as usize]
    }
}

/// Records with the hash field. Used for worker output and routing.
#[derive(Debug)]
pub struct MemBuffer {
    pub data: Vec<u8>,
}

impl MemBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        Self { data: Vec::with_capacity(cap) }
    }

    pub fn clear(&mut self) { self.data.clear(); }

    pub fn iter(&self) -> MemIter<'_> {
        MemIter { data: &self.data, offset: 0 }
    }

        /// Appends: size | count | hash | (ID|START)*count | sequence
    pub fn append_entry(&mut self, metas: &[(&str, u16)], sequence: &[u8], hash: u64) -> Result<EntryRef> {
        let count = u16::try_from(metas.len())?;
        let record_size = u32::try_from(COUNT_LEN + HASH_LEN + metas.len() * META_LEN + sequence.len())?;
        let start = self.data.len();

        self.data.extend_from_slice(&record_size.to_le_bytes());
        self.data.extend_from_slice(&count.to_le_bytes());
        self.data.extend_from_slice(&hash.to_le_bytes());
        for (id, pos) in metas {
            let mut id_bytes = [0u8; ID_LEN];
            let n = id.len().min(ID_LEN);
            id_bytes[..n].copy_from_slice(&id.as_bytes()[..n]);
            self.data.extend_from_slice(&id_bytes);
            self.data.extend_from_slice(&pos.to_le_bytes());
        }
        self.data.extend_from_slice(sequence);

        Ok(EntryRef { start: start as u64, len: (SIZE_LEN + record_size as usize) as u32 })
    }
}

/// Read view of one in-memory record.
impl EntryRef {
    pub fn count(&self, data: &[u8]) -> u16 {
        let o = self.start as usize + SIZE_LEN;
        u16::from_le_bytes([data[o], data[o + 1]])
    }

    pub fn get_meta_refs(&self, data: &[u8]) -> Vec<MetaRef> {
        let first = self.start + MEM_HEADER_LEN as u64;
        (0..self.count(data) as usize)
            .map(|i| MetaRef { start: first + (i * META_LEN) as u64, len: META_LEN as u32 })
            .collect()
    }

    pub fn get_seq_ref(&self, data: &[u8]) -> SeqRef {
        let prefix = MEM_HEADER_LEN as u32 + self.count(data) as u32 * META_LEN as u32;
        SeqRef { start: self.start + prefix as u64, len: self.len - prefix }
    }
}

pub struct MemIter<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for MemIter<'a> {
    type Item = EntryRef;

    fn next(&mut self) -> Option<EntryRef> {
        if self.offset + SIZE_LEN > self.data.len() { return None; }
        let start = self.offset;
        let record_size = u32::from_le_bytes(self.data[start..start + SIZE_LEN].try_into().ok()?) as usize;
        let len = SIZE_LEN + record_size;
        if record_size < COUNT_LEN + HASH_LEN || start + len > self.data.len() { return None; }
        self.offset = start + len;
        Some(EntryRef { start: start as u64, len: len as u32 })
    }
}

/// On-disk records: size | count | (ID|START)*count | sequence. No hash.
#[derive(Debug)]
pub struct DiskBuffer {
    pub data: Vec<u8>,
}

impl DiskBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        Self { data: Vec::with_capacity(cap) }
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn iter(&self) -> DiskIter<'_> {
        DiskIter { data: &self.data, offset: 0 }
    }

    /// Converts one in-memory record into the disk layout and appends it.
    pub fn push_from_mem(&mut self, mem: &MemBuffer, entry: EntryRef) -> Result<()> {
        let bytes = &mem.data[entry.start as usize..(entry.start + entry.len as u64) as usize];
        let record_size = u32::from_le_bytes(bytes[..SIZE_LEN].try_into()?) - HASH_LEN as u32;
        let hash_off = SIZE_LEN + COUNT_LEN;
        self.data.extend_from_slice(&record_size.to_le_bytes());
        self.data.extend_from_slice(&bytes[SIZE_LEN..hash_off]);
        self.data.extend_from_slice(&bytes[hash_off + HASH_LEN..]);
        Ok(())
    }
}

/// Read view of one on-disk record. Offsets are relative to the record start.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct DiskEntryRef {
    pub start: u64,
    pub len: u32,
}

impl DiskEntryRef {
    pub fn count(&self, data: &[u8]) -> u16 {
        let o = self.start as usize + SIZE_LEN;
        u16::from_le_bytes([data[o], data[o + 1]])
    }

    /// One MetaRef per (ID|START) pair, directly after the disk header.
    pub fn get_meta_refs(&self, data: &[u8]) -> Vec<MetaRef> {
        let first = self.start + DISK_HEADER_LEN as u64;
        (0..self.count(data) as usize)
            .map(|i| MetaRef { start: first + (i * META_LEN) as u64, len: META_LEN as u32 })
            .collect()
    }

    pub fn get_seq_ref(&self, data: &[u8]) -> SeqRef {
        let prefix = DISK_HEADER_LEN as u32 + self.count(data) as u32 * META_LEN as u32;
        SeqRef { start: self.start + prefix as u64, len: self.len - prefix }
    }
}

pub struct DiskIter<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for DiskIter<'a> {
    type Item = DiskEntryRef;

    fn next(&mut self) -> Option<DiskEntryRef> {
        if self.offset + SIZE_LEN > self.data.len() {
            return None;
        }
        let start = self.offset;
        let record_size = u32::from_le_bytes(self.data[start..start + SIZE_LEN].try_into().ok()?) as usize;
        let len = SIZE_LEN + record_size;
        if record_size < COUNT_LEN || start + len > self.data.len() {
            return None;
        }
        self.offset = start + len;
        Some(DiskEntryRef { start: start as u64, len: len as u32 })
    }
}