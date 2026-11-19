//! Whole run after conversion:
//!
//! 1. one pass over all graphs against all query intervals, dedup in memory; complete
//!    results go straight to the output, evicted ones to shard files
//! 2. dedup the shard files and append them to the output

mod run;
mod run_params;
mod run_report;

pub use run::run;
pub use run_params::RunParams;
pub use run_report::RunReport;
