//! Work items passed between the pipeline stages.

use std::sync::Arc;

use super::{Pass, PathState};
use crate::process_graphs::graph::ProteinGraph;

/// A graph read for a pass.
pub struct GraphMsg {
    pub graph: ProteinGraph,
    pub pass: Arc<Pass>,
}

/// A frontier path of a graph, continued depth first by a worker.
pub struct TraversalJob {
    pub graph: Arc<ProteinGraph>,
    pub path: Vec<PathState>,
    pub pass: Arc<Pass>,
}
