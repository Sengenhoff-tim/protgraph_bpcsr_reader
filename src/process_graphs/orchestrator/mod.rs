//! Long-lived traversal pipeline, run in passes (currently one pass with all intervals):
//!
//! ```text
//! reader ─graphs─▶ dispatchers ─jobs─▶ workers ─batches─▶ writers (dedup tables) ─▶ output / shards
//! ```
//!
//! Threads are spawned once ([`stages`]). A pass ends with a barrier
//! ([`ProcessPipeline::run_pass`]): when the last `Arc<Pass>` is dropped, every graph
//! of the pass was traversed; workers then flush their batches, and writers finish
//! the pass (complete entries to the output, the rest to the pass' shard files).
mod pass_report;
mod process_params;
mod process_pipeline;
mod sizing;
mod stages;

pub use pass_report::PassReport;
pub use process_params::ProcessParams;
pub use process_pipeline::ProcessPipeline;
