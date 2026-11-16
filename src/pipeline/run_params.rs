use std::path::PathBuf;

use crate::{parameters::Cli, process_graphs::ProcessParams};

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct RunParams {
    pub outdir: PathBuf,
    pub zip: bool,
    pub process: ProcessParams,
    /// memory for the final shard dedup
    pub dedup_budget: u64,
}

impl RunParams {
    /// Splits `avail_memory`: 60% dedup tables, 10% worker batches, 5% spill buffers
    /// during traversal; 70% for the final shard dedup (the tables are freed by then).
    pub fn from_cli(cli: &Cli) -> Self {
        let memory = cli.avail_memory as u64 * GIB;
        let share = |percent: u64| (memory / 100 * percent) as usize;
        Self {
            outdir: cli.outdir_path.clone(),
            zip: cli.zip,
            process: ProcessParams {
                threads: cli.avail_processors,
                max_vars: cli.max_vars,
                max_cleavages: cli.max_cleavages,
                bucket_size: cli.bucket_size as usize,
                table_budget: share(60),
                batch_budget: share(10),
                spill_budget: share(5),
                max_handles: cli.max_handles,
                jobs_per_graph: 10 * cli.avail_processors,
            },
            dedup_budget: memory / 100 * 70,
        }
    }
}
