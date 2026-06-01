use anyhow::{Error, Result, anyhow};

/// Bookkeeping for traversal
#[derive(Debug, Clone)]
pub struct State {
    pub parent: Option<u32>,
    pub node: u32,
    pub edge: Option<u32>,
    pub var: u8,
    pub tv: i64,
    pub previous: u32,
}

pub struct SubgraphForQuery {
    pub arena: Vec<State>,
    pub head_at_node: Vec<usize>,
}

impl SubgraphForQuery {
    pub fn new(limit: usize) -> Result<Self, Error> {
        let per_state = size_of::<State>() + size_of::<usize>();

        let max_states = limit / per_state;

        let mut arena = Vec::with_capacity(max_states);
        let mut head_at_node = Vec::new();

        // root state
        arena.push(State {
            parent: None,
            node: 0,
            edge: None,
            var: 0,
            tv: 0,
            previous: 0,
        });

        head_at_node.push(0usize);

        Ok(Self {
            arena,
            head_at_node,
        })
    }

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
        if self.arena.len() + 1 > self.arena.capacity() {
            return false;
        }

        let new_idx = self.arena.len() as u32;

        self.arena.push(State {
            parent,
            node,
            edge: Some(edge),
            var,
            tv,
            previous: self.head_at_node[target_node] as u32,
        });

        self.head_at_node[target_node] = new_idx as usize;

        true
    }

    pub fn clear(&mut self) {
        self.arena.clear();
        self.head_at_node.clear();

        // restore root state
        self.arena.push(State {
            parent: None,
            node: 0,
            edge: None,
            var: 0,
            tv: 0,
            previous: 0,
        });

        self.head_at_node.push(0usize);
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
}
