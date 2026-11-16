//! Batches of traversal results sent from workers to writers.

#[derive(Debug, Clone, Copy)]
struct RecRef {
    hash: u64,
    start: usize,
    len: u32,
    acc: u32,
}

/// One traversal result: packed sequence, its hash and the accession id it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemRecord<'a> {
    pub hash: u64,
    pub acc: u32,
    pub seq: &'a [u8],
}

/// Flat batch of records: all sequences in one byte vector, no allocation per record.
#[derive(Debug, Default)]
pub struct MemBuffer {
    seqs: Vec<u8>,
    recs: Vec<RecRef>,
}

impl MemBuffer {
    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            seqs: Vec::with_capacity(bytes / 2),
            recs: Vec::with_capacity(bytes / 2 / size_of::<RecRef>()),
        }
    }

    pub fn push(&mut self, hash: u64, acc: u32, seq: &[u8]) {
        self.recs.push(RecRef {
            hash,
            start: self.seqs.len(),
            len: seq.len() as u32,
            acc,
        });
        self.seqs.extend_from_slice(seq);
    }

    pub fn clear(&mut self) {
        self.seqs.clear();
        self.recs.clear();
    }

    /// Approximate memory held by the records.
    pub fn byte_len(&self) -> usize {
        self.seqs.len() + self.recs.len() * size_of::<RecRef>()
    }

    pub fn len(&self) -> usize {
        self.recs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = MemRecord<'_>> {
        self.recs.iter().map(|r| MemRecord {
            hash: r.hash,
            acc: r.acc,
            seq: &self.seqs[r.start..r.start + r.len as usize],
        })
    }
}
