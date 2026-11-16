//! Layout of a node block inside the graph arena.
//!
//! Every node owns one contiguous block, so all data needed when stepping onto a
//! node (weight, pdb intervals, outgoing edges) is a single random access:
//!
//! ```text
//! word 0        mono_weight (i64)
//! word 1        n_edges (bits 0..16) | n_pdbs (bits 16..32) | seq_len (bits 32..48)
//! 2 * n_pdbs    pdb intervals, (lower, upper) as i64, sorted by lower
//! ⌈n_edges/2⌉   edges, two u32 per word (low half first), see [`Edge`]
//! ⌈seq_len/8⌉   residue codes (see `aa_pack`), one per byte (low byte first)
//! ```

use crate::process_graphs::utilities::Interval;

use super::Edge;

pub(super) const HDR_WORDS: usize = 2;

#[inline]
pub(super) const fn block_words(n_edges: usize, n_pdbs: usize, seq_len: usize) -> usize {
    HDR_WORDS + 2 * n_pdbs + n_edges.div_ceil(2) + seq_len.div_ceil(8)
}

/// Word 1 of a block.
#[inline]
pub(super) const fn encode_counts(n_edges: usize, n_pdbs: usize, seq_len: usize) -> u64 {
    n_edges as u64 | (n_pdbs as u64) << 16 | (seq_len as u64) << 32
}

/// View on a single node block.
#[derive(Clone, Copy)]
pub struct Node<'a> {
    block: &'a [u64],
    n_edges: usize,
    n_pdbs: usize,
    seq_len: usize,
}

impl<'a> Node<'a> {
    /// Decodes the block starting at word `offset` of `arena`.
    #[inline]
    pub(super) fn at(arena: &'a [u64], offset: u32) -> Self {
        let o = offset as usize;
        let counts = arena[o + 1];
        let n_edges = (counts & 0xFFFF) as usize;
        let n_pdbs = ((counts >> 16) & 0xFFFF) as usize;
        let seq_len = ((counts >> 32) & 0xFFFF) as usize;
        Node {
            block: &arena[o..o + block_words(n_edges, n_pdbs, seq_len)],
            n_edges,
            n_pdbs,
            seq_len,
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
        (0..self.seq_len).map(move |i| (block[base + i / 8] >> (8 * (i % 8))) as u8)
    }

    /// True if any query interval, shifted by `offset`, overlaps a pdb interval of this node.
    /// Both lists are sorted by lower bound.
    #[inline]
    pub fn has_overlapping_interval(self, intervals: &[Interval], offset: i64) -> bool {
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
