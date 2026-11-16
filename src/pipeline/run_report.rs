use crate::deduplicate_output::OutputStats;

/// Summary of a whole run.
#[derive(Debug, Default, Clone, Copy)]
pub struct RunReport {
    pub intervals: usize,
    pub spilled_intervals: usize,
    pub output: OutputStats,
}
