use anyhow::Result;

use crate::process_graphs::utilities::{Interval, Pdbs, SubgraphForQuery};

/// relevant information for traversal
pub struct TraversalData {
    pub nodes: Box<[u32]>,
    pub edges: Box<[u32]>,
    pub mono_weight: Box<[i64]>,
    pub variant_count: Box<[u8]>,
    pub pdbs: Pdbs,
}

pub enum TraversalStatus {
    Success,
    Overflow,
}

/// Inner traversal function. This closely follows the original implementation.
impl TraversalData {
    pub fn traverse(
        &self,
        interval: Interval,
        max_vars: u8,
        traversal_state: &mut SubgraphForQuery,
    ) -> Result<TraversalStatus> {
        // clear subgraph
        traversal_state.reset(self.nodes.len());

        // subgraph is build in a single forward pass
        for node_idx in 0..self.nodes.len() - 1 {
            let edge_begin = if node_idx == 0 {
                0
            } else {
                self.nodes[node_idx - 1] as usize
            };

            let edge_end = self.nodes[node_idx] as usize;

            if traversal_state.states_at_node[node_idx].is_empty() {
                continue;
            }

            let current_states = traversal_state.states_at_node[node_idx].clone();

            for state_id in current_states {
                let state = traversal_state.arena[state_id].clone();
                let tv = state.tv;
                let var = state.var;

                for edge_idx in edge_begin..edge_end {
                    let new_var = var + self.variant_count[edge_idx];

                    if new_var > max_vars {
                        continue;
                    }

                    let target_node = self.edges[edge_idx] as usize;

                    let achieved = tv + self.mono_weight[target_node];

                    let lower = interval.lower - achieved;
                    let upper = interval.upper - achieved;

                    if !self.has_overlapping_interval(target_node, lower, upper) {
                        continue;
                    }

                    // If traversal state overflows 'limit', this returns 'overflow'.
                    if !traversal_state.push_state(
                        Some(state_id as u32),
                        self.edges[edge_idx],
                        edge_idx as u32,
                        new_var,
                        achieved,
                        target_node,
                    ) {
                        return Ok(TraversalStatus::Overflow);
                    }
                }
            }
        }
        Ok(TraversalStatus::Success)
    }

    // Check if intervals overlap. Intervals are not materialized for efficacy.
    #[inline]
    fn has_overlapping_interval(&self, node: usize, lower: i64, upper: i64) -> bool {
        let slice = match self.pdbs.get_node_intervals(node) {
            Some(s) => s,
            None => return false,
        };

        let mut i = 0;
        while i < slice.len() {
            let iv = unsafe { slice.get_unchecked(i) };

            if iv.lower > upper {
                break;
            }

            if iv.lower <= upper && iv.upper >= lower {
                return true;
            }

            i += 1;
        }

        false
    }
}
