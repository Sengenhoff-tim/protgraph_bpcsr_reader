use crossbeam_channel::Sender;

use super::TraversalContext;

/// One traversal pass over all graphs (one query interval).
///
/// Every graph and job of the pass holds an `Arc<Pass>`. When the last one is dropped,
/// all graphs of the pass have been read and traversed, and the pass index is sent on
/// `done`.
pub struct Pass {
    pub idx: usize,
    pub ctx: TraversalContext,
    done: Sender<usize>,
}

impl Pass {
    pub fn new(idx: usize, ctx: TraversalContext, done: Sender<usize>) -> Self {
        Self { idx, ctx, done }
    }
}

impl Drop for Pass {
    fn drop(&mut self) {
        let _ = self.done.send(self.idx);
    }
}
