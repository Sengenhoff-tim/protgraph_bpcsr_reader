//! The bucketed table itself, see the module docs.

use std::mem;

use anyhow::Result;

use super::{AccBytes, Entry, SeqBytes, TableConfig, TableStats, table_entry::contains_acc};
use crate::shared::acc_id::encode_acc;

pub struct DedupTable {
    cfg: TableConfig,
    /// 16 bits of each entry's hash, compared before the sequence
    tags: Box<[u16]>,
    entries: Box<[Entry]>,
    counts: Box<[u8]>,
    evicted: Box<[bool]>,
    /// buckets that became non-empty since the last drain (may contain repeats)
    touched: Vec<u32>,
    heap: usize,
    cursor: usize,
    stats: TableStats,
}

impl DedupTable {
    pub fn new(cfg: TableConfig) -> Self {
        assert!(cfg.bucket_size >= 1 && cfg.bucket_size <= u8::MAX as usize);
        assert!((1..=4).contains(&cfg.acc_width));
        assert!(cfg.n_buckets >= 1);
        let slots = cfg.n_buckets * cfg.bucket_size;
        Self {
            cfg,
            tags: vec![0; slots].into_boxed_slice(),
            entries: (0..slots).map(|_| Entry::default()).collect(),
            counts: vec![0; cfg.n_buckets].into_boxed_slice(),
            evicted: vec![false; cfg.n_buckets].into_boxed_slice(),
            touched: Vec::new(),
            heap: 0,
            cursor: 0,
            stats: TableStats::default(),
        }
    }

    pub fn config(&self) -> TableConfig {
        self.cfg
    }

    pub fn stats(&self) -> TableStats {
        self.stats
    }

    pub fn heap_bytes(&self) -> usize {
        self.heap
    }

    /// Bucket of a hash: the top bits, scaled to the bucket count (any count, no modulo).
    #[inline]
    fn bucket(&self, hash: u64) -> usize {
        ((hash as u128 * self.cfg.n_buckets as u128) >> 64) as usize
    }

    /// Tag of a hash. The writer is chosen by the low bits (`hash % n_writers`) and the
    /// bucket by the top bits, so the tag uses bits 16..32, independent of both.
    #[inline]
    fn tag(hash: u64) -> u16 {
        (hash >> 16) as u16
    }

    /// Adds the mapping `seq → acc` (a no-op if it exists). Evicted entries are handed to
    /// `evict` together with their bucket index.
    pub fn insert<F>(&mut self, hash: u64, seq: &[u8], acc: u32, evict: &mut F) -> Result<()>
    where
        F: FnMut(usize, Entry) -> Result<()>,
    {
        let acc = encode_acc(acc);
        let acc = &acc[..self.cfg.acc_width];
        let b = self.bucket(hash);
        let tag = Self::tag(hash);

        match self.find(b, tag, seq) {
            Some(i) => self.add_acc(b, i, acc),
            None => {
                if self.counts[b] as usize == self.cfg.bucket_size {
                    self.evict_at(b, self.cfg.bucket_size - 1, evict)?;
                }
                self.insert_front(b, tag, seq, acc);
            }
        }
        self.enforce_heap_budget(evict)
    }

    /// Position of `seq` within bucket `b`.
    #[inline]
    fn find(&self, b: usize, tag: u16, seq: &[u8]) -> Option<usize> {
        let base = b * self.cfg.bucket_size;
        (0..self.counts[b] as usize)
            .find(|&i| self.tags[base + i] == tag && self.entries[base + i].seq.as_slice() == seq)
    }

    /// Hit: records `acc` if new and moves the entry to the front of its bucket.
    fn add_acc(&mut self, b: usize, i: usize, acc: &[u8]) {
        let base = b * self.cfg.bucket_size;
        let entry = &mut self.entries[base + i];
        if !contains_acc(&entry.accs, acc) {
            let before = entry.heap_bytes();
            entry.accs.extend_from_slice(acc);
            self.heap += entry.heap_bytes() - before;
        }
        self.tags[base..=base + i].rotate_right(1);
        self.entries[base..=base + i].rotate_right(1);
        self.stats.hits += 1;
    }

    /// New sequence at the front of bucket `b`, which must have a free slot.
    fn insert_front(&mut self, b: usize, tag: u16, seq: &[u8], acc: &[u8]) {
        let base = b * self.cfg.bucket_size;
        let n = self.counts[b] as usize;
        // the free slot at base + n moves to the front
        self.tags[base..=base + n].rotate_right(1);
        self.entries[base..=base + n].rotate_right(1);
        self.tags[base] = tag;
        self.entries[base] = Entry {
            seq: SeqBytes::from_slice(seq),
            accs: AccBytes::from_slice(acc),
        };
        self.heap += self.entries[base].heap_bytes();
        if n == 0 {
            self.touched.push(b as u32);
        }
        self.counts[b] += 1;
        self.stats.inserted += 1;
    }

    /// Removes entry `i` of bucket `b`, marks the bucket and hands the entry to `evict`.
    fn evict_at<F>(&mut self, b: usize, i: usize, evict: &mut F) -> Result<()>
    where
        F: FnMut(usize, Entry) -> Result<()>,
    {
        let base = b * self.cfg.bucket_size;
        let (slot, end) = (base + i, base + self.counts[b] as usize);
        let entry = mem::take(&mut self.entries[slot]);
        // close the gap; the emptied slot moves behind the last entry
        self.tags[slot..end].rotate_left(1);
        self.entries[slot..end].rotate_left(1);
        self.heap -= entry.heap_bytes();
        self.counts[b] -= 1;
        self.evicted[b] = true;
        self.stats.evicted += 1;
        evict(b, entry)
    }

    /// Evicts entries with heap storage, bucket by bucket, until the heap fits the budget.
    fn enforce_heap_budget<F>(&mut self, evict: &mut F) -> Result<()>
    where
        F: FnMut(usize, Entry) -> Result<()>,
    {
        while self.heap > self.cfg.heap_budget {
            let b = self.cursor;
            let base = b * self.cfg.bucket_size;
            match (0..self.counts[b] as usize)
                .rev()
                .find(|&i| self.entries[base + i].heap_bytes() > 0)
            {
                Some(i) => self.evict_at(b, i, evict)?,
                None => self.cursor = (self.cursor + 1) % self.cfg.n_buckets,
            }
        }
        Ok(())
    }

    /// Empties the table. `out` receives every entry with its bucket index and whether its
    /// bucket evicted since the last drain: entries of such buckets may have copies outside
    /// the table, all others are complete.
    pub fn drain<F>(&mut self, mut out: F) -> Result<()>
    where
        F: FnMut(usize, Entry, bool) -> Result<()>,
    {
        let k = self.cfg.bucket_size;
        for b in mem::take(&mut self.touched) {
            let b = b as usize;
            let base = b * k;
            let evicted = self.evicted[b];
            for slot in base..base + self.counts[b] as usize {
                out(b, mem::take(&mut self.entries[slot]), evicted)?;
            }
            self.counts[b] = 0;
            self.evicted[b] = false;
        }
        self.heap = 0;
        self.cursor = 0;
        Ok(())
    }
}
