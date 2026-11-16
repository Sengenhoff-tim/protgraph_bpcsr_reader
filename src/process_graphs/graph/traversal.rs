use anyhow::Result;

use crate::process_graphs::utilities::{PathState, TraversalContext};

use super::{Edge, ProteinGraph};

impl ProteinGraph {
    /// Returns the state after stepping over `edge`, or `None` if the edge is pruned.
    #[inline]
    pub fn process_edge(
        &self,
        edge: Edge,
        ctx: &TraversalContext,
        current: &PathState,
    ) -> Option<PathState> {
        let cleaves = current.cleaves + edge.cleaves();
        if cleaves > ctx.max_missed_cleavages {
            return None;
        }

        let var = current.var + edge.features();
        if var > ctx.max_variants {
            return None;
        }

        let target = edge.target();
        let node = self.node(target);
        let tv = current.tv + node.mono_weight();

        node.has_overlapping_interval(&ctx.intervals, tv)
            .then_some(PathState {
                node: target,
                var,
                cleaves,
                tv,
            })
    }

    pub fn traverse_depth_first(
        &self,
        ctx: &TraversalContext,
        traversal_state: &mut Vec<PathState>,
        on_complete: &mut impl FnMut(&[PathState]) -> Result<()>,
    ) -> Result<()> {
        let current = *traversal_state
            .last()
            .expect("path must contain at least the root");

        if current.node == self.terminal() {
            return on_complete(traversal_state);
        }

        for edge in self.node(current.node).edges() {
            if let Some(next) = self.process_edge(edge, ctx, &current) {
                traversal_state.push(next);
                self.traverse_depth_first(ctx, traversal_state, on_complete)?;
                traversal_state.pop();
            }
        }
        Ok(())
    }
}
