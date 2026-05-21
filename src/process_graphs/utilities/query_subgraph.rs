/// Bookkeeping for traversal
#[derive(Debug, Clone)]
pub struct State {
    pub parent: Option<usize>,
    pub node: u32,
    pub edge: Option<u32>,
    pub var: u8,
    pub tv: i64,
}

const STATE_SIZE: usize = size_of::<State>();

pub enum TraversalStatus {
    Complete(SubgraphForQuery),
    Overflow(),
}

pub struct SubgraphForQuery {
    pub limit: usize,
    pub arena: Vec<State>,
    pub states_at_node: Vec<Vec<usize>>,
}

impl SubgraphForQuery {
    pub fn new(num_nodes: usize, max_vars: u8, limit: usize) -> Self {
        let mut arena = Vec::with_capacity(num_nodes * max_vars as usize);
        let mut states_at_node = vec![Vec::new(); num_nodes];

        arena.push(State {
            parent: None,
            node: 0,
            edge: None,
            var: 0,
            tv: 0,
        });

        states_at_node[0].push(0_usize);

        Self {
            limit,
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

            state_id = state.parent;
        }

        trace.reverse();
        trace
    }

    /// Checks for memory overflow since traversal state grows exponentially.
    #[inline]
    pub fn push_state(
        &mut self,
        parent: Option<usize>,
        node: u32,
        edge: u32,
        var: u8,
        tv: i64,
        target_node: usize,
    ) -> bool {
        let len: usize = self.arena.len();

        if self.arena.len() * STATE_SIZE >= self.limit {
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
