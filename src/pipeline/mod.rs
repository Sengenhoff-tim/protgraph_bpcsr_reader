//! Whole run after conversion:
//!
//! 1. for every query interval: traverse all graphs, dedup in memory; complete results go
//!    straight to the output, evicted ones to the interval's shard files
//! 2. dedup the shard files of all intervals that spilled and append them to the output
//!
//! Query intervals are disjoint and a peptide's weight follows from its sequence, so a
//! sequence never occurs in two intervals and per-interval dedup is global dedup.

mod run;
mod run_params;
mod run_report;

pub use run::run;
pub use run_params::RunParams;
pub use run_report::RunReport;
