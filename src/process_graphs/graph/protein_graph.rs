use std::{collections::VecDeque, sync::Arc};

use crate::process_graphs::utilities::{PathState, TraversalContext, TraversalJob};

use super::{MetaData, TreeData};

pub struct ProteinGraph {
    pub tree_data: TreeData,
    pub meta_data: MetaData,
}

impl ProteinGraph {
    pub fn build_frontier(
        self: &Arc<Self>,
        ctx: &TraversalContext,
        target: usize,
    ) -> Vec<TraversalJob> {
        let tree = &self.tree_data;
        let terminal = tree.nodes.len() - 1;

        let mut open: VecDeque<Vec<PathState>> = VecDeque::new();
        let mut done: Vec<Vec<PathState>> = Vec::new();

        open.push_back(vec![PathState {
            node: 0,
            edge: None,
            var: 0,
            cleaves: 0,
            tv: 0,
        }]);

        while open.len() < target {
            let Some(path) = open.pop_front() else { break };
            let tip = *path.last().expect("path must contain at least the root");

            if tip.node as usize == terminal {
                done.push(path);
                continue;
            }

            let node = tip.node as usize;
            let edge_begin = if node == 0 { 0 } else { tree.nodes[node - 1] as usize };
            let edge_end = tree.nodes[node] as usize;

            for (offset, &edge) in tree.ft_clv_edge[edge_begin..edge_end].iter().enumerate() {
                let edge_idx = edge_begin + offset;
                if let Some((target_node, var, cleaves, tv)) = tree.process_edge(edge, ctx, &tip) {
                    let mut child = path.clone();
                    child.push(PathState {
                        node: target_node,
                        edge: Some(edge_idx as u32),
                        var,
                        cleaves,
                        tv,
                    });
                    if target_node as usize == terminal {
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