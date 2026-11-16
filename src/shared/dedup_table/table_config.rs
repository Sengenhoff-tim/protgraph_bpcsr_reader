//! Table geometry derived from a memory budget.

use super::Entry;

/// Share of the budget used for the fixed slot array; the rest is the heap budget.
const FIXED_SHARE_PERCENT: usize = 80;
const MAX_BUCKET_BITS: u32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableConfig {
    pub bucket_bits: u32,
    pub bucket_size: usize,
    pub heap_budget: usize,
    pub acc_width: usize,
}

impl TableConfig {
    /// Memory of one slot: hash plus entry.
    pub const SLOT_BYTES: usize = size_of::<u64>() + size_of::<Entry>();

    /// Derives the bucket count from a memory budget: 80% for the slot array (rounded
    /// down to a power of two buckets), the remainder for heap overflow.
    pub fn from_budget(budget: usize, bucket_size: usize, acc_width: usize) -> Self {
        let bucket_size = bucket_size.clamp(1, u8::MAX as usize);
        let fixed = budget / 100 * FIXED_SHARE_PERCENT;
        let n_buckets = (fixed / (bucket_size * Self::SLOT_BYTES)).max(1);
        let bucket_bits = n_buckets.ilog2().min(MAX_BUCKET_BITS);
        Self {
            bucket_bits,
            bucket_size,
            heap_budget: budget - fixed,
            acc_width,
        }
    }

    pub fn n_buckets(&self) -> usize {
        1 << self.bucket_bits
    }

    /// Memory of the slot array.
    pub fn fixed_bytes(&self) -> usize {
        self.n_buckets() * self.bucket_size * Self::SLOT_BYTES
    }
}
