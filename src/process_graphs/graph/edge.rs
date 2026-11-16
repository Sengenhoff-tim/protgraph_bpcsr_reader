//! Packed edge: `target offset (28 bit) | features (3 bit) | cleave (1 bit)`.

use anyhow::{Result, bail, ensure};

const TARGET_SHIFT: u32 = 4;
/// Edge targets are stored with 28 bits.
pub const MAX_OFFSET: usize = (1 << (32 - TARGET_SHIFT)) - 1;
/// Feature counts are stored with 3 bits.
pub const MAX_FEATURES: u8 = 0b111;

/// Edge as stored in a node block. The target is the word offset of the target block.
#[derive(Clone, Copy, Debug)]
pub struct Edge(pub(super) u32);

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

/// Edge as handed to [`ProteinGraph::build`](super::ProteinGraph::build). The target is a node index.
#[derive(Clone, Copy, Debug)]
pub struct RawEdge {
    pub target: u32,
    pub features: u8,
    pub cleave: bool,
}

impl RawEdge {
    /// Packs the edge, resolving the target node index through the block `offsets`.
    pub(super) fn encode(&self, offsets: &[u32]) -> Result<Edge> {
        let Some(&target) = offsets.get(self.target as usize) else {
            bail!("edge target {} out of range", self.target);
        };
        ensure!(
            self.features <= MAX_FEATURES,
            "edge feature count {} exceeds {MAX_FEATURES}",
            self.features
        );
        Ok(Edge(
            target << TARGET_SHIFT | (self.features as u32) << 1 | self.cleave as u32,
        ))
    }
}
