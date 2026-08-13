use anyhow::Result;

use crate::process_graphs::utilities::{Interval, Pdbs, TraversalState, PathState};

/// relevant information for traversal
pub struct TraversalData {
    pub nodes: Box<[u32]>,
    pub mono_weight: Box<[i64]>,
    pub ft_clv_edge: Box<[(u16, u16, u32)]>,
    pub pdbs: Pdbs,
}

pub enum TraversalStatus {
    Success,
    Overflow,
}

fn dfs(
    data: &TraversalData,
    interval: Interval,
    max_vars: u16,
    max_cleaves: u16,
    traversal_state: &mut TraversalState,
    on_complete: &mut impl FnMut(&[PathState], u16) -> Result<()>,
) -> Result<TraversalStatus> {
    let (node, tv, cleaves, var) = {
        let s = traversal_state.current();
        (s.node, s.tv, s.cleaves, s.var)
    };

    let node_idx = node as usize;
    let last_idx = data.nodes.len() - 1;

    if node_idx == last_idx {
        // stack IS the complete root->leaf path, already in order.
        on_complete(traversal_state.path(), cleaves)?;
        return Ok(TraversalStatus::Success);
    }

    let edge_begin = if node_idx == 0 { 0 } else { data.nodes[node_idx - 1] as usize };
    let edge_end = data.nodes[node_idx] as usize;

    for (index, &(edge_ft_count, edge_cleave, target)) in
        data.ft_clv_edge[edge_begin..edge_end].iter().enumerate()
    {
        let cur_cleaves = cleaves + edge_cleave;
        if cur_cleaves > max_cleaves { continue; }

        let cur_var = var + edge_ft_count;
        if cur_var > max_vars { continue; }

        let cur_tv = tv + data.mono_weight[target as usize];

        let lower = interval.lower - cur_tv;
        let upper = interval.upper - cur_tv;

        if !data.has_overlapping_interval(target as usize, lower, upper) {
            continue;
        }

        if !traversal_state.push_state(
            target,
            (edge_begin + index) as u32,
            cur_var,
            cur_cleaves,
            cur_tv,
        ) {
            return Ok(TraversalStatus::Overflow);
        }

        let status = dfs(data, interval, max_vars, max_cleaves, traversal_state, on_complete)?;

        traversal_state.pop_state();

        if let TraversalStatus::Overflow = status {
            return Ok(TraversalStatus::Overflow);
        }
    }

    Ok(TraversalStatus::Success)
}

/// Inner traversal function. This closely follows the original implementation.
impl TraversalData {
    /* 
    pub fn traverse(
        &self,
        interval: Interval,
        max_vars: u16,
        max_cleaves: u16,
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

            let current_states = std::mem::take(&mut traversal_state.states_at_node[node_idx]);

            for state_id in current_states {
                let (tv, cleaves, var) = {
                    let node = &traversal_state.arena[state_id];
                    (node.tv, node.cleaves, node.var)
                };

                for (index, &(edge_ft_count, edge_cleave, target)) in
                    self.ft_clv_edge[edge_begin..edge_end].iter().enumerate()
                {
                    let cur_cleaves = cleaves + edge_cleave;

                    if cur_cleaves > max_cleaves {
                        continue;
                    }

                    let cur_var = var + edge_ft_count;
                    if cur_var > max_vars {
                        continue;
                    }

                    let cur_tv = tv + self.mono_weight[target as usize];

                    let lower = interval.lower - cur_tv;
                    let upper = interval.upper - cur_tv;

                    if !self.has_overlapping_interval(target as usize, lower, upper) {
                        continue;
                    }

                    // If traversal state overflows 'limit', this returns 'overflow'.
                    if !traversal_state.push_state(
                        Some(state_id as u32),
                        target,
                        (edge_begin + index) as u32,
                        cur_var,
                        cur_cleaves,
                        cur_tv,
                    ) {
                        return Ok(TraversalStatus::Overflow);
                    }
                }
            }
        }
        Ok(TraversalStatus::Success)
    }
*/
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

            if iv.upper >= lower {
                return true;
            }

            i += 1;
        }

        false
    }
    
}

impl TraversalData {
    pub fn traverse(
        &self,
        interval: Interval,
        max_vars: u16,
        max_cleaves: u16,
        traversal_state: &mut TraversalState,
        mut on_complete: impl FnMut(&[PathState], u16) -> Result<()>,
    ) -> Result<TraversalStatus> {
        traversal_state.reset();
        dfs(self, interval, max_vars, max_cleaves, traversal_state, &mut on_complete)
    }
}