/// Pdbs are stored in a vector of intervals.
/// The relevant intervals are stored in a continuous vector and can be retrived by node index.
use anyhow::{Result, anyhow};

use crate::process_graphs::utilities::Interval;

pub struct Pdbs {
    pub all: Vec<Interval>,
    pub offsets: Vec<usize>,
}

impl Pdbs {
    pub fn from_node_lists(node_lists: Vec<Vec<Interval>>) -> Result<Self> {
        let n_nodes = node_lists.len();

        let mut total = 0usize;
        for v in &node_lists {
            total = total
                .checked_add(v.len())
                .ok_or_else(|| anyhow!("size overflow"))?;
        }

        let mut all = Vec::with_capacity(total);
        let mut offsets = Vec::with_capacity(n_nodes + 1);
        offsets.push(0);

        for v in node_lists {
            all.extend_from_slice(&v);
            offsets.push(all.len());
        }

        Ok(Pdbs { all, offsets })
    }

    pub fn get_node_intervals(&self, node: usize) -> Option<&[Interval]> {
        if node + 1 >= self.offsets.len() {
            return None;
        }
        let s = self.offsets[node];
        let e = self.offsets[node + 1];
        Some(&self.all[s..e])
    }
}
