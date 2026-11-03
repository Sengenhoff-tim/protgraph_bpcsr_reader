/// A metadata entry attached to a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub id: String,
    pub pos: u16,
}

/// A worker output record. Holds structured values only, no byte layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub metas: Vec<Meta>,
    pub sequence: Vec<u8>,
    pub hash: u64,
}

/// In-memory collection of records used for worker output and routing.
#[derive(Debug, Default)]
pub struct MemBuffer {
    records: Vec<Record>,
    bytes: usize,
}

impl MemBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            records: Vec::with_capacity(cap),
            bytes: 0,
        }
    }

    pub fn push(&mut self, record: Record) {
        self.bytes += record.sequence.len();
        self.records.push(record);
    }

    pub fn clear(&mut self) {
        self.records.clear();
        self.bytes = 0;
    }

    /// Approximate size: sum of sequence lengths held.
    pub fn byte_len(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Record> {
        self.records.iter()
    }
}