use std::mem::size_of;

/// A single node in the currently-live DFS path. No `parent` field —
/// ordering/ancestry is implicit in the stack position (Vec index).
#[derive(Debug, Clone, Copy)]
pub struct PathState {
    pub node: u32,
    pub edge: Option<u32>,
    pub var: u16,
    pub cleaves: u16,
    pub tv: i64,
}

/// Live DFS path, maintained as a strict stack: push on descent, pop on
/// backtrack. At any point in traversal, `stack[0..]` IS the current
/// root-to-here path, already in order — no reconstruction needed.
pub struct TraversalState {
    stack: Vec<PathState>,
    max_states: usize,
}

impl TraversalState {
    pub fn new(limit: usize) -> Self {
        let max_states = limit / size_of::<PathState>();
        Self {
            stack: Vec::with_capacity(max_states),
            max_states,
        }
    }

    /// Resets to a fresh traversal starting at the root node.
    pub fn reset(&mut self) {
        self.stack.clear();
        self.stack.push(PathState {
            node: 0,
            edge: None,
            var: 0,
            cleaves: 0,
            tv: 0,
        });
    }

    /// Pushes a child state. Returns false if the configured limit is hit.
    #[inline]
    pub fn push_state(
        &mut self,
        node: u32,
        edge: u32,
        var: u16,
        cleaves: u16,
        tv: i64,
    ) -> bool {
        if self.stack.len() == self.max_states {
            return false;
        }

        self.stack.push(PathState {
            node,
            edge: Some(edge),
            var,
            cleaves,
            tv,
        });

        true
    }

    /// Undoes the last push — LIFO, mirrors DFS backtracking.
    #[inline]
    pub fn pop_state(&mut self) {
        self.stack.pop();
    }

    /// Current top-of-stack state (the node DFS is presently standing on).
    #[inline]
    pub fn current(&self) -> &PathState {
        self.stack.last().expect("traversal state is never empty after reset")
    }

    /// The full live path, root -> current, already in forward order.
    #[inline]
    pub fn path(&self) -> &[PathState] {
        &self.stack
    }
}