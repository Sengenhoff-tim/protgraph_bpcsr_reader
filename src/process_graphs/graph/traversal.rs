use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Arc;

use crate::process_graphs::{
    graph::ProteinGraph, 
    utilities::{Interval, PathState, Pdbs, TraversalState, TraversalJob},
};

/// relevant information for traversal
pub struct TreeData {
    pub nodes: Box<[u32]>,
    pub mono_weight: Box<[i64]>,
    pub ft_clv_edge: Box<[(u16, u16, u32)]>,
    pub pdbs: Pdbs,
}

struct EdgeContext<'a> {
    tree: &'a TreeData,
    intervals: &'a [Interval],
    max_cleaves: u16,
    max_vars: u16,
}

fn process_edge(
    ctx: &EdgeContext,
    edge_idx: usize,
    current_state: &PathState,
) -> Option<(u32, u16, u16, i64)> {
    let (edge_ft_count, edge_cleave, target) = ctx.tree.ft_clv_edge[edge_idx];
    
    let cur_cleaves = current_state.cleaves + edge_cleave;
    if cur_cleaves > ctx.max_cleaves {
        return None;
    }

    let cur_var = current_state.var + edge_ft_count;
    if cur_var > ctx.max_vars {
        return None;
    }

    let cur_tv = current_state.tv + ctx.tree.mono_weight[target as usize];

    if ctx.tree.has_overlapping_interval(target as usize, ctx.intervals, cur_tv as i64) {
        return Some((target, cur_var, cur_cleaves, cur_tv));
    }
    None
}


/// Builds up to `target` traversal jobs in BFS order.
///
/// Expansion goes through `process_edge`, the same rule `dfs` uses, so jobs
/// cover the same state space as a full traversal.
pub struct FrontierBuilder {
    graph: Arc<ProteinGraph>,
    intervals: Arc<[Interval]>,
    max_vars: u16,
    max_cleaves: u16,
    target: usize,
}

impl FrontierBuilder {
    pub fn new(
        graph: Arc<ProteinGraph>,
        intervals: Arc<[Interval]>,
        max_vars: u16,
        max_cleaves: u16,
        target: usize,
    ) -> Self {
        debug_assert!(target > 0, "FrontierBuilder: target must be > 0, got {target}");
        debug_assert!(
            !intervals.is_empty(),
            "FrontierBuilder: intervals must not be empty; with no query intervals no path can be matched"
        );
        debug_assert!(
            graph.tree_data.nodes.len() >= 2,
            "FrontierBuilder: graph needs at least a start and a terminal node, got {} node entries",
            graph.tree_data.nodes.len()
        );
        debug_assert!(
            intervals.windows(2).all(|w| w[0].lower <= w[1].lower),
            "FrontierBuilder: intervals must be sorted by lower bound, since has_overlapping_interval relies on it"
        );
        Self { graph, intervals, max_vars, max_cleaves, target }
    }

    pub fn build(&self) -> Vec<TraversalJob> {
        let tree: &TreeData = &self.graph.tree_data;
        let terminal = tree.nodes.len() - 1;
        let ctx = EdgeContext {
            tree,
            intervals: &self.intervals,
            max_cleaves: self.max_cleaves,
            max_vars: self.max_vars,
        };

        let mut open: VecDeque<Vec<PathState>> = VecDeque::new();
        let mut done: Vec<Vec<PathState>> = Vec::new();
        open.push_back(TraversalState::new().path().to_vec());

        while open.len() < self.target {
            let Some(path) = open.pop_front() else { break };
            let tip = *path.last().expect("path is never empty");

            if tip.node as usize == terminal {
                done.push(path);
                continue;
            }

            let node = tip.node as usize;
            let edge_begin = if node == 0 { 0 } else { tree.nodes[node - 1] as usize };
            let edge_end = tree.nodes[node] as usize;

            for edge_idx in edge_begin..edge_end {
                if let Some((target, var, cleaves, tv)) = process_edge(&ctx, edge_idx, &tip) {
                    let mut child = path.clone();
                    child.push(PathState {
                        node: target,
                        edge: Some(edge_idx as u32),
                        var,
                        cleaves,
                        tv,
                    });
                    // Children that reach the terminal node go to `done` right away, so
                    // they never occupy a slot in `open`.
                    if target as usize == terminal {
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
                graph: Arc::clone(&self.graph),
                intervals: Arc::clone(&self.intervals),
                state: TraversalState::from_path(path),
            })
            .collect()
    }
}

fn dfs(
    ctx: &EdgeContext,
    traversal_state: &mut TraversalState,
    on_complete: &mut impl FnMut(&[PathState], u16) -> Result<()>,
) -> Result<()> {
    let current_state = *traversal_state.current();
    let node_idx = current_state.node as usize;
    let tree = ctx.tree;
    let last_idx = tree.nodes.len() - 1;

    if node_idx == last_idx {
        on_complete(traversal_state.path(), current_state.cleaves)?;
        return Ok(());
    }

    let edge_begin = if node_idx == 0 { 0 } else { tree.nodes[node_idx - 1] as usize };
    let edge_end = tree.nodes[node_idx] as usize;

    for (index, _) in tree.ft_clv_edge[edge_begin..edge_end].iter().enumerate() {
        let edge_idx = edge_begin + index;
        if let Some((target, cur_var, cur_cleaves, cur_tv)) = process_edge(&ctx, edge_idx, &current_state) {
            traversal_state.push_state(target, edge_idx as u32, cur_var, cur_cleaves, cur_tv);
            dfs(&ctx, traversal_state, on_complete)?;
            traversal_state.pop_state();
        }
    }
    Ok(())
}

impl TreeData {
    /// Check if any interval in the slice overlaps with node intervals.
    #[inline]
    fn has_overlapping_interval(&self, node: usize, intervals: &[Interval], offset: i64) -> bool {
        let Some(slice) = self.pdbs.get_node_intervals(node) else { return false };
        let mut start = 0;
        for q in intervals {
            let lower = q.lower - offset;
            let upper = q.upper - offset;
            // Node intervals ending before `lower` can't match this or any later query.
            while start < slice.len() && slice[start].upper < lower {
                start += 1;
            }
            if start < slice.len() && slice[start].lower <= upper {
                return true;
            }
        }
        false
    }
}

impl TreeData {
    pub fn traverse(
        &self,
        intervals: &[Interval],
        max_vars: u16,
        max_cleaves: u16,
        traversal_state: &mut TraversalState,
        mut on_complete: impl FnMut(&[PathState], u16) -> Result<()>,
    ) -> Result<()> {
        let ctx = EdgeContext{
            tree: self,
            intervals,
            max_cleaves,
            max_vars
        };

        dfs(&ctx, traversal_state, &mut on_complete)
    }
}