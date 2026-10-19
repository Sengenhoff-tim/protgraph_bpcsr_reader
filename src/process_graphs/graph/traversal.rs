use anyhow::Result;

use crate::process_graphs::utilities::{Interval, Pdbs, TraversalState, PathState};

/// relevant information for traversal
pub struct TraversalData {
    pub nodes: Box<[u32]>,
    pub mono_weight: Box<[i64]>,
    pub ft_clv_edge: Box<[(u16, u16, u32)]>,
    pub pdbs: Pdbs,
}

fn dfs(
    data: &TraversalData,
    intervals: &[Interval],
    max_vars: u16,
    max_cleaves: u16,
    traversal_state: &mut TraversalState,
    on_complete: &mut impl FnMut(&[PathState], u16) -> Result<()>,
    offload: &mut impl FnMut(&[PathState]) -> bool
) -> Result<()> {
    let (node, tv, cleaves, var) = {
        let s = traversal_state.current();
        (s.node, s.tv, s.cleaves, s.var)
    };

    let node_idx = node as usize;
    let last_idx = data.nodes.len() - 1;

    if node_idx == last_idx {
        on_complete(traversal_state.path(), cleaves)?;
        return Ok(());
    }

    let edge_begin = if node_idx == 0 { 0 } else { data.nodes[node_idx - 1] as usize };
    let edge_end = data.nodes[node_idx] as usize;

    for (index, &(edge_ft_count, edge_cleave, target))  in
        data.ft_clv_edge[edge_begin..edge_end].iter().enumerate()
    {
        let cur_cleaves = cleaves + edge_cleave;
        if cur_cleaves > max_cleaves { continue; }

        let cur_var = var + edge_ft_count;
        if cur_var > max_vars { continue; }

        let cur_tv = tv + data.mono_weight[target as usize];

        let mut overlap = false;
        for interval in intervals {
            let lower = interval.lower - cur_tv;
            let upper = interval.upper - cur_tv;

            if data.has_overlapping_interval(target as usize, lower, upper) {
                overlap = true;
                break;
            }
        }
        if !overlap {
            continue;
        }
                

        traversal_state.push_state(
            target,
            (edge_begin + index) as u32,
            cur_var,
            cur_cleaves,
            cur_tv,
        );

        let handed_off = offload(traversal_state.path());

        if !handed_off {
            dfs(data, intervals, max_vars, max_cleaves, traversal_state,
                on_complete, offload)?;
        }

        traversal_state.pop_state();
    }
    Ok(())
}

/// Inner traversal function. This closely follows the original implementation.
impl TraversalData {
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
        intervals: &[Interval],
        max_vars: u16,
        max_cleaves: u16,
        traversal_state: &mut TraversalState,
        offload: &mut impl FnMut(&[PathState]) -> bool,
        mut on_complete: impl FnMut(&[PathState], u16) -> Result<()>,
    ) -> Result<()> {
        dfs(self, intervals, max_vars, max_cleaves, traversal_state,
            &mut on_complete, offload)
    }
}