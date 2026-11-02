// src/entry.rs

pub const SIZE_LEN: usize = 4;
pub const ID_LEN: usize = 10;
pub const START_LEN: usize = 2;
/// Bytes before the sequence: record size, ID, start position.
pub const HEADER_LEN: usize = SIZE_LEN + ID_LEN + START_LEN;
/// Bytes covered by MetaRef: ID and start position.
pub const META_LEN: usize = ID_LEN + START_LEN;

#[derive(Debug)]
pub struct EntryBuffer {
    pub data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct EntryRef {
    pub start: u64,
    pub len: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SeqRef {
    pub start: u64,
    pub len: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct MetaRef {
    pub start: u64,
    pub len: u32,
}

impl MetaRef {
    #[inline]
    pub fn get<'a>(&self, buffer: &'a EntryBuffer) -> &'a [u8] {
        let start = self.start as usize;
        &buffer.data[start..start + self.len as usize]
    }
}

impl SeqRef {
    #[inline]
    pub fn get<'a>(&self, buffer: &'a EntryBuffer) -> &'a [u8] {
        let start = self.start as usize;
        &buffer.data[start..start + self.len as usize]
    }
}

impl EntryBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: Vec::with_capacity(cap),
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn iter(&self) -> BinEntryIterator<'_> {
        BinEntryIterator {
            data: &self.data,
            offset: 0,
        }
    }
}

impl EntryRef {
    #[inline]
    pub fn get<'a>(&self, buffer: &'a EntryBuffer) -> &'a [u8] {
        let start = self.start as usize;
        &buffer.data[start..start + self.len as usize]
    }

    /// Returns the ID and start position bytes, excluding the size field and sequence.
    #[inline]
    pub fn get_meta_ref(&self) -> MetaRef {
        MetaRef {
            start: self.start + SIZE_LEN as u64,
            len: META_LEN as u32,
        }
    }

    /// Returns the packed 5-bit sequence range.
    #[inline]
    pub fn get_seq_ref(&self) -> SeqRef {
        SeqRef {
            start: self.start + HEADER_LEN as u64,
            len: self.len - HEADER_LEN as u32,
        }
    }
}

pub struct BinEntryIterator<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for BinEntryIterator<'a> {
    type Item = EntryRef;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset + SIZE_LEN > self.data.len() {
            return None;
        }
        let start = self.offset;
        let record_size =
            u32::from_le_bytes(self.data[start..start + SIZE_LEN].try_into().ok()?) as usize;

        // record_size counts the bytes after itself, so the full record is SIZE_LEN + record_size.
        let len = SIZE_LEN + record_size;
        let end = start + len;
        if record_size < HEADER_LEN - SIZE_LEN || end > self.data.len() {
            return None;
        }

        self.offset = end;
        Some(EntryRef {
            start: start as u64,
            len: len as u32,
        })
    }
}
