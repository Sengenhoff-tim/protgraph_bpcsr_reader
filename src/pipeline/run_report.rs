use crate::deduplicate_output::OutputStats;

/// Summary of a whole run.
#[derive(Debug, Default, Clone, Copy)]
pub struct RunReport {
    pub intervals: usize,
    /// whether entries were spilled to disk and deduplicated from there
    pub spilled: bool,
    pub output: OutputStats,
}
