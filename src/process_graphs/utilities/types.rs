use std::sync::Arc;

use crate::process_graphs::{graph::ProteinGraph, utilities::Interval};

#[derive(Debug, Clone)]
pub struct TraversalContext {
    pub max_variants: u16,
    pub max_missed_cleavages: u16,
    pub intervals: Box<[Interval]>,
}

#[derive(Debug, Clone, Copy)]
pub struct PathState {
    /// word offset of the node block in the graph arena
    pub node: u32,
    pub var: u16,
    pub cleaves: u16,
    pub tv: i64,
}

pub struct TraversalJob {
    pub graph: Arc<ProteinGraph>,
    pub path: Vec<PathState>,
}
