//! Long-lived traversal pipeline, run once per query interval:
//!
//! ```text
//! reader ─graphs─▶ dispatchers ─jobs─▶ workers ─batches─▶ writers (dedup tables) ─▶ output / shards
//! ```
//!
//! Threads are spawned once ([`stages`]). An interval ends with a barrier
//! ([`ProcessPipeline::run_interval`]): when the last `Arc<Pass>` is dropped, every graph
//! of the interval was traversed; workers then flush their batches, and writers finish
//! the interval (complete entries to the output, the rest to the interval's shard files).
mod interval_report;
mod process_params;
mod process_pipeline;
mod sizing;
mod stages;

pub use interval_report::IntervalReport;
pub use process_params::ProcessParams;
pub use process_pipeline::ProcessPipeline;
