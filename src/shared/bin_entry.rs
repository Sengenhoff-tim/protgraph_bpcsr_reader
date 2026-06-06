pub struct EntryBuffer {
    pub data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct EntryRef {
    pub start: u64,
    pub len: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SeqRef{
    pub start: u64,
    pub len: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct MetaRef{
    pub start: u64,
    pub len: u32,
}

impl EntryRef {
    #[inline]
    pub fn get<'a>(&self, buffer: &'a EntryBuffer) -> &'a [u8] {
        let start = self.start as usize;
        let end = start + self.len as usize;

        &buffer.data[start..end]
    }

    #[inline]
    pub fn get_seq<'a>(&self, buffer: &'a EntryBuffer) -> &'a [u8] {
        let entry_start = self.start as usize;

        let mut pos = entry_start + 4;

        let seq_len = u32::from_le_bytes(
            buffer.data[pos..pos + 4]
                .try_into()
                .unwrap(),
        ) as usize;

        pos += 4;

        &buffer.data[pos..pos + seq_len]
    }

    #[inline]
    pub fn get_seq_meta_ref(&self, buffer: &[u8]) -> (SeqRef, MetaRef) {
        let entry_start = self.start as usize;
        let entry_end = entry_start + self.len as usize;

        let mut pos = entry_start + 4; // skip record_size

        let seq_len = u32::from_le_bytes(
            buffer[pos..pos + 4]
                .try_into()
                .expect("invalid entry"),
        ) as usize;

        pos += 4;

        let seq = SeqRef {
            start: pos as u64,
            len: seq_len as u32,
        };

        pos += seq_len;

        let meta = MetaRef {
            start: pos as u64,
            len: (entry_end - pos) as u32,
        };

        (seq, meta)
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

impl SeqRef {
    #[inline]
    pub fn get<'a>(&self, buffer: &'a EntryBuffer) -> &'a [u8] {
        let start = self.start as usize;
        let end = start + self.len as usize;

        &buffer.data[start..end]
    }
}

impl MetaRef {
    #[inline]
    pub fn get<'a>(&self, buffer: &'a EntryBuffer) -> &'a [u8] {
        let start = self.start as usize;
        let end = start + self.len as usize;

        &buffer.data[start..end]
    }
}

pub struct BinEntryIterator<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for BinEntryIterator<'a> {
    type Item = EntryRef;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            return None;
        }

        let start = self.offset;

        let record_size = u32::from_le_bytes(
            self.data[start..start + 4]
                .try_into()
                .ok()?,
        ) as usize;

        let len = 4 + record_size;

        let end = start + len;

        if end > self.data.len() {
            return None;
        }

        self.offset = end;

        Some(EntryRef {
            //TODO make nicer
            start: start as u64 +4,
            len: len as u32,
        })
    }
}