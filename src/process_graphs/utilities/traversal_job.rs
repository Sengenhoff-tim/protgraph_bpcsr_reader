use std::sync::Arc;

use crate::process_graphs::{graph::ProteinGraph, utilities::{Interval, TraversalState}};

pub enum Msg {
    Job(TraversalJob),
    Stop,
}

pub struct TraversalJob {
    pub graph: Arc<ProteinGraph>,
    pub intervals: Arc<[Interval]>,
    pub state: TraversalState,
}
