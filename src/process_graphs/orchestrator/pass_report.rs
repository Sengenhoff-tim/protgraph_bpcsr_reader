use crate::{process_graphs::threading::WriterReport, shared::TableStats};

/// Outcome of one pass, summed over all writers.
#[derive(Debug, Default, Clone, Copy)]
pub struct PassReport {
    /// whether shard files were written for this pass
    pub spilled: bool,
    /// peptides written directly to the output
    pub direct: u64,
    pub stats: TableStats,
}

impl PassReport {
    pub(super) fn add(&mut self, writer: WriterReport) {
        self.spilled |= writer.spilled;
        self.direct += writer.direct;
        self.stats.inserted += writer.stats.inserted;
        self.stats.hits += writer.stats.hits;
        self.stats.evicted += writer.stats.evicted;
    }
}
