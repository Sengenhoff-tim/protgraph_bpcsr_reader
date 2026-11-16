//! Splitting a graph into independent traversal jobs.

use std::{collections::VecDeque, sync::Arc};

use crate::process_graphs::utilities::{PathState, TraversalContext, TraversalJob};

use super::{ProteinGraph, protein_graph::ROOT};

impl ProteinGraph {
    /// Expands paths breadth first from the root until at least `target` jobs exist
    /// (or the graph is exhausted). Each job continues depth first from its path.
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
