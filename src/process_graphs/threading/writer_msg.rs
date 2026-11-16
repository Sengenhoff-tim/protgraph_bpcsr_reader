use crossbeam_channel::Sender;

use crate::shared::{MemBuffer, TableStats};

pub enum WriterMsg {
    Batch(MemBuffer),
    /// Finish the current interval and acknowledge with the writer's report.
    EndInterval(Sender<WriterReport>),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct WriterReport {
    /// whether anything of this interval was written to shard files
    pub spilled: bool,
    /// peptides sent directly to the output
    pub direct: u64,
    pub stats: TableStats,
}
