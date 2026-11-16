use super::Interval;

/// Limits and query intervals a traversal checks against.
#[derive(Debug, Clone)]
pub struct TraversalContext {
    pub max_variants: u16,
    pub max_missed_cleavages: u16,
    pub intervals: Box<[Interval]>,
}
