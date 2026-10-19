#[derive(Debug, Clone, Copy)]
pub struct PathState {
    pub node: u32,
    pub edge: Option<u32>,
    pub var: u16,
    pub cleaves: u16,
    pub tv: i64,
}

pub struct TraversalState {
    stack: Vec<PathState>,
}

impl TraversalState {
    pub fn new() -> Self {
        let mut s = Self {
            stack: Vec::new(),
        };
        s.stack.push(PathState {
            node: 0,
            edge: None,
            var: 0,
            cleaves: 0,
            tv: 0,
        });
        s
    }

    pub fn from_path(path: Vec<PathState>) -> Self {
        assert!(!path.is_empty(), "path must contain at least the root");
        Self { stack: path }
    }

    /* 
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
    */
    
    /// Pushes a child state. Returns false if the configured limit is hit.
    #[inline]
    pub fn push_state(
        &mut self,
        node: u32,
        edge: u32,
        var: u16,
        cleaves: u16,
        tv: i64,
    ) {
        self.stack.push(PathState {
            node,
            edge: Some(edge),
            var,
            cleaves,
            tv,
        });
    }

    #[inline]
    pub fn pop_state(&mut self) {
        self.stack.pop();
    }

    #[inline]
    pub fn current(&self) -> &PathState {
        self.stack.last().expect("TraversalState is empty")
    }

    #[inline]
    pub fn path(&self) -> &[PathState] {
        &self.stack
    }
}