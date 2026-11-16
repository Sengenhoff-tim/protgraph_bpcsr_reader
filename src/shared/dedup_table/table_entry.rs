//! One table entry: a sequence and its accession ids.

use smallvec::SmallVec;

/// Inline up to 24 packed bytes (38 residues).
pub type SeqBytes = SmallVec<[u8; 24]>;
/// Inline up to 16 bytes of accession ids (4..16 ids depending on the id width).
pub type AccBytes = SmallVec<[u8; 16]>;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Entry {
    pub seq: SeqBytes,
    pub accs: AccBytes,
}

impl Entry {
    /// Bytes allocated outside the inline storage.
    #[inline]
    pub(super) fn heap_bytes(&self) -> usize {
        let seq = if self.seq.spilled() {
            self.seq.capacity()
        } else {
            0
        };
        let accs = if self.accs.spilled() {
            self.accs.capacity()
        } else {
            0
        };
        seq + accs
    }
}

/// True if the id list `accs` contains `acc`. Checks the most recent id first: paths of
/// one graph arrive together, so repeats are almost always of the last id.
#[inline]
pub(super) fn contains_acc(accs: &[u8], acc: &[u8]) -> bool {
    accs.ends_with(acc) || accs.chunks_exact(acc.len()).any(|c| c == acc)
}
