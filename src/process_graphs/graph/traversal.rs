use anyhow::Result;

use crate::process_graphs::{
    utilities::{FtClvEdge, PathState, Pdbs, TraversalContext},
};

/// relevant information for traversal
pub struct TreeData {
    pub nodes: Box<[u32]>,
    pub mono_weight: Box<[i64]>,
    pub ft_clv_edge: Box<[FtClvEdge]>,
    pub pdbs: Pdbs,
}

impl TreeData {
    pub fn process_edge(&self, edge: FtClvEdge, ctx: &TraversalContext, current: &PathState) -> Option<(u32, u16, u16, i64)> {

        let cur_cleaves = current.cleaves + edge.cleaves;
        if cur_cleaves > ctx.max_missed_cleavages {
            return None;
        }

        let cur_var = current.var + edge.features;
        if cur_var > ctx.max_variants {
            return None;
        }

        let target = edge.edge;
        let cur_tv = current.tv + self.mono_weight[target as usize];

        if self.pdbs.has_overlapping_interval(target as usize, &ctx.intervals, cur_tv) {
            return Some((target, cur_var, cur_cleaves, cur_tv));
        }
        None
    }

    pub fn traverse_depth_first(
        &self,
        ctx: &TraversalContext,
        traversal_state: &mut Vec<PathState>,
        on_complete: &mut impl FnMut(&[PathState], u16) -> Result<()>,
    ) -> Result<()> {
        let current_state = *traversal_state.last().expect("path must contain at least the root");
        let node_idx = current_state.node as usize;
        let last_idx = self.nodes.len() - 1;

        if node_idx == last_idx {
            on_complete(traversal_state, current_state.cleaves)?;
            return Ok(());
        }

        let edge_begin = if node_idx == 0 { 0 } else { self.nodes[node_idx - 1] as usize };
        let edge_end = self.nodes[node_idx] as usize;

        for (offset, &edge) in self.ft_clv_edge[edge_begin..edge_end].iter().enumerate() {
            let edge_idx = edge_begin + offset;
            if let Some((target, cur_var, cur_cleaves, cur_tv)) =
                self.process_edge(edge, &ctx, &current_state)
            {
                traversal_state.push(PathState {
                    node: target,
                    edge: Some(edge_idx as u32),
                    var: cur_var,
                    cleaves: cur_cleaves,
                    tv: cur_tv,
                });
                self.traverse_depth_first(ctx, traversal_state, on_complete)?;
                traversal_state.pop();
            }
        }
        Ok(())
    }
}