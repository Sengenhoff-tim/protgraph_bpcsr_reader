//! Packed edge: `target offset (28 bit) | features (3 bit) | cleave (1 bit)`.
//!
//! Feature counts are capped at [`FEATURE_CAP`]: a stored cap means "more than any allowed
//! `max_vars`", so such edges are always pruned, exactly like with their real count.

use anyhow::{Result, bail};

const FEATURE_BITS: u32 = 3;
const FEATURE_MASK: u32 = (1 << FEATURE_BITS) - 1;
const TARGET_SHIFT: u32 = 1 + FEATURE_BITS;
/// Edge targets are stored with 28 bits (2 GiB of arena per graph).
pub const MAX_OFFSET: usize = (1 << (32 - TARGET_SHIFT)) - 1;
/// Feature counts of this value or more are stored as this value.
pub const FEATURE_CAP: u8 = FEATURE_MASK as u8;
/// Largest `max_vars` the edge format supports exactly: one below the cap.
pub const MAX_VARIANTS: u16 = FEATURE_CAP as u16 - 1;

/// Edge as stored in a node block. The target is the word offset of the target block.
#[derive(Clone, Copy, Debug)]
pub struct Edge(pub(super) u32);

impl Edge {
    #[inline]
    pub fn target(self) -> u32 {
        self.0 >> TARGET_SHIFT
    }

    /// Feature count, capped at [`FEATURE_CAP`].
    #[inline]
    pub fn features(self) -> u16 {
        ((self.0 >> 1) & FEATURE_MASK) as u16
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
        let features = self.features.min(FEATURE_CAP) as u32;
        Ok(Edge(
            target << TARGET_SHIFT | features << 1 | self.cleave as u32,
        ))
    }
}
