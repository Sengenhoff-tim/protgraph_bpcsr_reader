//! Table geometry derived from a memory budget.

use super::Entry;

/// Share of the budget used for the fixed slot array; the rest is the heap budget.
const FIXED_SHARE_PERCENT: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableConfig {
    pub n_buckets: usize,
    pub bucket_size: usize,
    pub heap_budget: usize,
    pub acc_width: usize,
}

impl TableConfig {
    /// Memory of one slot: 16-bit tag plus entry.
    pub const SLOT_BYTES: usize = size_of::<u16>() + size_of::<Entry>();

    /// Derives the bucket count from a memory budget: 80% for the slot array, the
    /// remainder for heap overflow.
    pub fn from_budget(budget: usize, bucket_size: usize, acc_width: usize) -> Self {
        let bucket_size = bucket_size.clamp(1, u8::MAX as usize);
        let fixed = budget / 100 * FIXED_SHARE_PERCENT;
        Self {
            n_buckets: (fixed / (bucket_size * Self::SLOT_BYTES)).max(1),
            bucket_size,
            heap_budget: budget - fixed,
            acc_width,
        }
    }

    /// Memory of the slot array.
    pub fn fixed_bytes(&self) -> usize {
        self.n_buckets * self.bucket_size * Self::SLOT_BYTES
    }
}
