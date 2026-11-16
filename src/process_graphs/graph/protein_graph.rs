//! Protein graph stored as a single arena of `u64` words.
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
//!
//! Nodes are addressed by the word offset of their block. The root is always at
//! offset 0.

use std::{collections::VecDeque, sync::Arc};

use anyhow::{Result, bail, ensure};

use crate::{
    process_graphs::utilities::{
        Interval, PathState, TraversalContext, TraversalJob, pack_codes_into,
    },
    shared::Meta,
};

const HDR_WORDS: usize = 2;
const TARGET_SHIFT: u32 = 4;
/// Edge targets are stored with 28 bits.
pub const MAX_OFFSET: usize = (1 << (32 - TARGET_SHIFT)) - 1;
/// Feature counts are stored with 3 bits.
pub const MAX_FEATURES: u8 = 0b111;

pub const ROOT: u32 = 0;

/// Packed edge: `target offset (28 bit) | features (3 bit) | cleave (1 bit)`.
#[derive(Clone, Copy, Debug)]
pub struct Edge(u32);

impl Edge {
    #[inline]
    pub fn target(self) -> u32 {
        self.0 >> TARGET_SHIFT
    }

    #[inline]
    pub fn features(self) -> u16 {
        ((self.0 >> 1) & MAX_FEATURES as u32) as u16
    }

    #[inline]
    pub fn cleaves(self) -> u16 {
        (self.0 & 1) as u16
    }
}

/// View on a single node block.
#[derive(Clone, Copy)]
pub struct Node<'a> {
    block: &'a [u64],
    n_edges: usize,
    n_pdbs: usize,
    seq_len: usize,
}

#[inline]
const fn block_words(n_edges: usize, n_pdbs: usize, seq_len: usize) -> usize {
    HDR_WORDS + 2 * n_pdbs + n_edges.div_ceil(2) + seq_len.div_ceil(8)
}

impl<'a> Node<'a> {
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

/// Node as handed to [`ProteinGraph::build`]. Edge targets are node indices.
pub struct RawNode<'a> {
    pub mono_weight: i64,
    pub pdbs: &'a [Interval],
    pub edges: &'a [RawEdge],
    /// residue codes, see `aa_pack::encode_residue`
    pub seq: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct RawEdge {
    pub target: u32,
    pub features: u8,
    pub cleave: bool,
}

pub struct ProteinGraph {
    accession: String,
    arena: Box<[u64]>,
    terminal: u32,
}

impl ProteinGraph {
    /// Lays out the nodes into the arena. Node 0 is the root, the last node the terminal.
    pub fn build(accession: String, nodes: &[RawNode]) -> Result<Self> {
        ensure!(!nodes.is_empty(), "graph has no nodes");

        // pass 1: block offsets
        let mut offsets = Vec::with_capacity(nodes.len());
        let mut total = 0usize;
        for (i, node) in nodes.iter().enumerate() {
            ensure!(
                node.edges.len() <= u16::MAX as usize
                    && node.pdbs.len() <= u16::MAX as usize
                    && node.seq.len() <= u16::MAX as usize,
                "node {i} exceeds 65535 edges, pdbs or residues"
            );
            ensure!(
                total <= MAX_OFFSET,
                "graph too large: node {i} at word offset {total} exceeds 28 bit addressing"
            );
            offsets.push(total as u32);
            total += block_words(node.edges.len(), node.pdbs.len(), node.seq.len());
        }

        // pass 2: write blocks
        let mut arena = Vec::with_capacity(total);
        for node in nodes {
            arena.push(node.mono_weight as u64);
            arena.push(
                node.edges.len() as u64
                    | (node.pdbs.len() as u64) << 16
                    | (node.seq.len() as u64) << 32,
            );
            for iv in node.pdbs {
                arena.push(iv.lower as u64);
                arena.push(iv.upper as u64);
            }
            for pair in node.edges.chunks(2) {
                let mut word = 0u64;
                for (j, edge) in pair.iter().enumerate() {
                    word |= (encode_edge(edge, &offsets)? as u64) << (32 * j);
                }
                arena.push(word);
            }
            for chunk in node.seq.chunks(8) {
                let mut word = 0u64;
                for (j, &code) in chunk.iter().enumerate() {
                    word |= (code as u64) << (8 * j);
                }
                arena.push(word);
            }
        }
        debug_assert_eq!(arena.len(), total);

        Ok(Self {
            accession,
            arena: arena.into_boxed_slice(),
            terminal: *offsets.last().expect("non-empty"),
        })
    }

    /// Reassembles a graph from its serialized parts.
    pub fn from_parts(accession: String, arena: Box<[u64]>, terminal: u32) -> Result<Self> {
        ensure!(
            (terminal as usize) < arena.len(),
            "terminal offset {terminal} outside arena of {} words",
            arena.len()
        );
        Ok(Self {
            accession,
            arena,
            terminal,
        })
    }

    pub fn accession(&self) -> &str {
        &self.accession
    }

    pub fn arena(&self) -> &[u64] {
        &self.arena
    }

    pub fn terminal(&self) -> u32 {
        self.terminal
    }

    #[inline]
    pub fn node(&self, offset: u32) -> Node<'_> {
        let o = offset as usize;
        let hdr = self.arena[o + 1];
        let n_edges = (hdr & 0xFFFF) as usize;
        let n_pdbs = ((hdr >> 16) & 0xFFFF) as usize;
        let seq_len = ((hdr >> 32) & 0xFFFF) as usize;
        Node {
            block: &self.arena[o..o + block_words(n_edges, n_pdbs, seq_len)],
            n_edges,
            n_pdbs,
            seq_len,
        }
    }

    /// Metas attached to a record.
    pub fn build_metas(&self, _trace: &[PathState]) -> Vec<Meta> {
        vec![Meta {
            id: self.accession.clone(),
            pos: u16::MAX,
        }]
    }

    /// Packed sequence of the path, excluding root and terminal.
    pub fn build_sequence(&self, trace: &[PathState], sequence_buffer: &mut Vec<u8>) {
        if trace.len() < 2 {
            sequence_buffer.clear();
            return;
        }
        pack_codes_into(
            sequence_buffer,
            trace[1..trace.len() - 1]
                .iter()
                .flat_map(|state| self.node(state.node).seq()),
        );
    }

    pub fn build_frontier(
        self: &Arc<Self>,
        ctx: &TraversalContext,
        target: usize,
    ) -> Vec<TraversalJob> {
        let mut open: VecDeque<Vec<PathState>> = VecDeque::new();
        let mut done: Vec<Vec<PathState>> = Vec::new();

        open.push_back(vec![PathState {
            node: ROOT,
            var: 0,
            cleaves: 0,
            tv: 0,
        }]);

        while open.len() < target {
            let Some(path) = open.pop_front() else { break };
            let tip = *path.last().expect("path must contain at least the root");

            if tip.node == self.terminal {
                done.push(path);
                continue;
            }

            for edge in self.node(tip.node).edges() {
                if let Some(next) = self.process_edge(edge, ctx, &tip) {
                    let mut child = path.clone();
                    child.push(next);
                    if next.node == self.terminal {
                        done.push(child);
                    } else {
                        open.push_back(child);
                    }
                }
            }
        }

        open.into_iter()
            .chain(done)
            .map(|path| TraversalJob {
                graph: Arc::clone(self),
                path,
            })
            .collect()
    }
}

fn encode_edge(edge: &RawEdge, offsets: &[u32]) -> Result<u32> {
    let Some(&target) = offsets.get(edge.target as usize) else {
        bail!("edge target {} out of range", edge.target);
    };
    ensure!(
        edge.features <= MAX_FEATURES,
        "edge feature count {} exceeds {MAX_FEATURES}",
        edge.features
    );
    Ok(target << TARGET_SHIFT | (edge.features as u32) << 1 | edge.cleave as u32)
}
