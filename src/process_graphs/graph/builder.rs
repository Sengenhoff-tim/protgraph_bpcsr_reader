//! Lays out nodes into the arena, see [`node`](super::node) for the block layout.

use anyhow::{Result, ensure};

use crate::process_graphs::utilities::Interval;

use super::{
    ProteinGraph, RawEdge,
    edge::MAX_OFFSET,
    node::{block_words, encode_counts},
};

/// Node as handed to [`ProteinGraph::build`]. Edge targets are node indices.
pub struct RawNode<'a> {
    pub mono_weight: i64,
    pub pdbs: &'a [Interval],
    pub edges: &'a [RawEdge],
    /// residue codes, see `aa_pack::encode_residue`
    pub seq: &'a [u8],
}

impl ProteinGraph {
    /// Lays out the nodes into the arena. Node 0 is the root, the last node the terminal.
    pub fn build(acc_id: u32, nodes: &[RawNode]) -> Result<Self> {
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
            arena.push(encode_counts(
                node.edges.len(),
                node.pdbs.len(),
                node.seq.len(),
            ));
            for iv in node.pdbs {
                arena.push(iv.lower as u64);
                arena.push(iv.upper as u64);
            }
            for pair in node.edges.chunks(2) {
                let mut word = 0u64;
                for (j, edge) in pair.iter().enumerate() {
                    word |= (edge.encode(&offsets)?.0 as u64) << (32 * j);
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
            acc_id,
            arena: arena.into_boxed_slice(),
            terminal: *offsets.last().expect("non-empty"),
        })
    }
}
