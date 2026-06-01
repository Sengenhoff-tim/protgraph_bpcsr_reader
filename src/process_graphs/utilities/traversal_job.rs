use std::sync::Arc;

use crate::process_graphs::{graph::ProteinGraph, utilities::Interval};

pub enum TraversalWorkerResult {
    Complete,
    Reschedule(TraversalJob),
}

pub struct TraversalJob {
    pub graph: Arc<ProteinGraph>,
    pub interval: Interval,
    pub depth: usize,
}
