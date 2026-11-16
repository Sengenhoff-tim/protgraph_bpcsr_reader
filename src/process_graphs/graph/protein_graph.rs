//! Protein graph stored as a single arena of `u64` words, one block per node
//! (see [`node`](super::node) for the block layout).
//!
//! Nodes are addressed by the word offset of their block. The root is always at
//! offset 0.

use anyhow::{Result, ensure};

use super::Node;

pub const ROOT: u32 = 0;

pub struct ProteinGraph {
    /// id in the global accession table
    pub(super) acc_id: u32,
    pub(super) arena: Box<[u64]>,
    pub(super) terminal: u32,
}

impl ProteinGraph {
    /// Reassembles a graph from its serialized parts.
    pub fn from_parts(acc_id: u32, arena: Box<[u64]>, terminal: u32) -> Result<Self> {
        ensure!(
            (terminal as usize) < arena.len(),
            "terminal offset {terminal} outside arena of {} words",
            arena.len()
        );
        Ok(Self {
            acc_id,
            arena,
            terminal,
        })
    }

    pub fn acc_id(&self) -> u32 {
        self.acc_id
    }

    pub fn arena(&self) -> &[u64] {
        &self.arena
    }

    pub fn terminal(&self) -> u32 {
        self.terminal
    }

    #[inline]
    pub fn node(&self, offset: u32) -> Node<'_> {
        Node::at(&self.arena, offset)
    }
}
