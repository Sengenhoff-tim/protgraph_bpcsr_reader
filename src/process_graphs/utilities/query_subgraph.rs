use anyhow::{Error, Result};

/// Bookkeeping for traversal

#[derive(Debug, Clone)]
pub struct State {
    pub parent: Option<u32>,
    pub node: u32,
    pub edge: Option<u32>,
    pub var: u8,
    pub tv: i64,
}

pub struct SubgraphForQuery {
    pub arena: Vec<State>,
    pub states_at_node: Vec<Vec<usize>>,
}

impl SubgraphForQuery {
    pub fn new(limit: usize) -> Self {
        let per_state = size_of::<State>() + size_of::<usize>();

        let max_states = limit / per_state;

        let arena = Vec::with_capacity(max_states);
        let states_at_node = Vec::new();

        Self {
            arena,
            states_at_node,
        }
    }

    /// Reconstructs a single path starting from any node.
    pub fn reconstruct_trace(&self, state_idx: usize) -> Vec<(u32, Option<u32>)> {
        let mut trace = Vec::new();
        let mut state_id = Some(state_idx);

        while let Some(id) = state_id {
            let state = &self.arena[id];

            trace.push((state.node, state.edge));

            state_id = state.parent.map(|v| v as usize);
        }

        trace.reverse();
        trace
    }

    /// Checks for memory overflow since traversal state grows exponentially.
    #[inline]
    pub fn push_state(
        &mut self,
        parent: Option<u32>,
        node: u32,
        edge: u32,
        var: u8,
        tv: i64,
        target_node: usize,
    ) -> bool {
        let len: usize = self.arena.len();

        if self.arena.len() == self.arena.capacity() {
            return false;
        }

        self.arena.push(State {
            parent,
            node,
            edge: Some(edge),
            var,
            tv,
        });

        self.states_at_node[target_node].push(len);

        true
    }
}

impl SubgraphForQuery {
    pub fn reset(&mut self, num_nodes: usize) {
        self.arena.clear();

        self.states_at_node.clear();
        self.states_at_node.resize_with(num_nodes, Vec::new);
   
        self.arena.push(State {
            parent: None,
            node: 0,
            edge: None,
            var: 0,
            tv: 0,
        });

        self.states_at_node[0].push(0_usize);
    }
}
