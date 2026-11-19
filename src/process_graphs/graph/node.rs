//! Layout of a node block inside the graph arena.
//!
//! Every node owns one contiguous block, so all data needed when stepping onto a
//! node (weight, pdb intervals, outgoing edges) is a single random access:
//!
//! ```text
//! word 0             mono_weight (i64)
//! word 1             n_edges (bits 0..32) | n_pdbs (bits 32..64)
//! 2 * n_pdbs         pdb intervals, (lower, upper) as i64, sorted by lower
//! ⌈n_edges/2⌉        edges, two u32 per word (low half first), see [`Edge`]
//! ⌈(4 + seq_len)/8⌉  seq_len (u32), then residue codes (see `aa_pack`), one per byte;
//!                    bytes in little-endian order within each word
//! ```
//!
//! The sequence length sits in the sequence section, which only output building reads,
//! so traversal never touches it.

use crate::process_graphs::utilities::Interval;

use super::Edge;

pub(super) const HDR_WORDS: usize = 2;
/// Bytes of the sequence length prefix.
const SEQ_LEN_BYTES: usize = 4;
/// Up to this many pdb intervals, the overlap check binary-searches the queries.
const BINARY_SEARCH_MAX_PDBS: usize = 16;

/// Largest edge count of a node (the root links to every peptide start).
pub const MAX_NODE_EDGES: usize = u32::MAX as usize;
/// Largest pdb interval count of a node (scales with the edges as well).
pub const MAX_NODE_PDBS: usize = u32::MAX as usize;
/// Largest residue count of a node.
pub const MAX_NODE_RESIDUES: usize = u32::MAX as usize;

const PDBS_SHIFT: u32 = 32;

#[inline]
pub(super) const fn block_words(n_edges: usize, n_pdbs: usize, seq_len: usize) -> usize {
    HDR_WORDS + 2 * n_pdbs + n_edges.div_ceil(2) + (SEQ_LEN_BYTES + seq_len).div_ceil(8)
}

/// Word 1 of a block. Counts must be within the `MAX_NODE_*` limits.
#[inline]
pub(super) const fn encode_counts(n_edges: usize, n_pdbs: usize) -> u64 {
    n_edges as u64 | (n_pdbs as u64) << PDBS_SHIFT
}

/// Words of the sequence section: length prefix, then the residue codes.
pub(super) fn encode_seq(seq: &[u8]) -> impl Iterator<Item = u64> + '_ {
    let len = (seq.len() as u32).to_le_bytes();
    let bytes: Vec<u8> = len.iter().chain(seq).copied().collect();
    (0..bytes.len().div_ceil(8)).map(move |w| {
        bytes[w * 8..(w * 8 + 8).min(bytes.len())]
            .iter()
            .enumerate()
            .fold(0u64, |word, (j, &b)| word | (b as u64) << (8 * j))
    })
}

/// View on a single node block.
#[derive(Clone, Copy)]
pub struct Node<'a> {
    /// the arena from this block's start on
    block: &'a [u64],
    n_edges: usize,
    n_pdbs: usize,
}

impl<'a> Node<'a> {
    /// Decodes the block starting at word `offset` of `arena`.
    #[inline]
    pub(super) fn at(arena: &'a [u64], offset: u32) -> Self {
        let o = offset as usize;
        let counts = arena[o + 1];
        Node {
            block: &arena[o..],
            n_edges: (counts & u32::MAX as u64) as usize,
            n_pdbs: (counts >> PDBS_SHIFT) as usize,
        }
    }

    #[inline]
    pub fn mono_weight(self) -> i64 {
        self.block[0] as i64
    }

    #[inline]
    fn pdb(self, i: usize) -> Interval {
        let w = HDR_WORDS + 2 * i;
        Interval {
            lower: self.block[w] as i64,
            upper: self.block[w + 1] as i64,
        }
    }

    /// pdb intervals as (lower, upper)
    #[cfg(test)]
    pub fn pdbs(self) -> impl Iterator<Item = (i64, i64)> + 'a {
        (0..self.n_pdbs).map(move |i| {
            let iv = self.pdb(i);
            (iv.lower, iv.upper)
        })
    }

    #[inline]
    pub fn edges(self) -> impl Iterator<Item = Edge> + 'a {
        let base = HDR_WORDS + 2 * self.n_pdbs;
        let block = self.block;
        (0..self.n_edges).map(move |i| Edge((block[base + i / 2] >> (32 * (i & 1))) as u32))
    }

    /// Residue codes of this node.
    #[inline]
    pub fn seq(self) -> impl Iterator<Item = u8> + 'a {
        let base = HDR_WORDS + 2 * self.n_pdbs + self.n_edges.div_ceil(2);
        let block = self.block;
        let byte = move |i: usize| (block[base + i / 8] >> (8 * (i % 8))) as u8;
        let len = u32::from_le_bytes([byte(0), byte(1), byte(2), byte(3)]) as usize;
        (SEQ_LEN_BYTES..SEQ_LEN_BYTES + len).map(byte)
    }

    /// True if any query interval, shifted by `offset`, overlaps a pdb interval of this node.
    /// `intervals` must be sorted and disjoint (as produced by `merge_and_sort`), so they
    /// are ordered by their upper bounds as well.
    #[inline]
    pub fn has_overlapping_interval(self, intervals: &[Interval], offset: i64) -> bool {
        // few node intervals (the normal case): binary search per node interval, which
        // stays cheap with thousands of queries
        if self.n_pdbs <= BINARY_SEARCH_MAX_PDBS {
            return (0..self.n_pdbs).any(|i| {
                let pdb = self.pdb(i);
                // first query not ending before the node interval starts
                let first = intervals.partition_point(|q| q.upper - offset < pdb.lower);
                intervals
                    .get(first)
                    .is_some_and(|q| q.lower - offset <= pdb.upper)
            });
        }
        // many node intervals: one merge-like sweep over both sorted lists
        let mut start = 0;
        for q in intervals {
            let lower = q.lower - offset;
            let upper = q.upper - offset;
            // Node intervals ending before `lower` can't match this or any later query.
            while start < self.n_pdbs && self.pdb(start).upper < lower {
                start += 1;
            }
            if start < self.n_pdbs && self.pdb(start).lower <= upper {
                return true;
            }
        }
        false
    }
}
