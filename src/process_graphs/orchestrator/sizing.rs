//! Thread counts, queue depths and buffer sizes derived from [`ProcessParams`].

use tracing::info;

use super::ProcessParams;
use crate::{process_graphs::threading::MIN_SPILL_BUF, shared::TableConfig};

const MIN_BATCH_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) struct Sizing {
    /// workers and dispatchers
    pub threads: usize,
    pub n_writers: usize,
    /// bound of the graph and job channels
    pub queue_depth: usize,
    /// batch buffers: one per worker and writer, plus a spare per writer in flight
    pub pool_size: usize,
    pub batch_bytes: usize,
    pub table: TableConfig,
    /// spill files per writer and interval
    pub n_shards: usize,
    pub spill_buf: usize,
}

impl Sizing {
    pub fn new(params: &ProcessParams, acc_width: usize) -> Self {
        let threads = params.threads.max(1);
        let n_writers = 2 * threads;
        let pool_size = threads * n_writers + n_writers;
        let handles = params.max_handles.unwrap_or_else(sys_open_files) as usize;
        let n_shards = (handles / n_writers).max(1);
        Self {
            threads,
            n_writers,
            queue_depth: threads * 2,
            pool_size,
            batch_bytes: (params.batch_budget / pool_size).max(MIN_BATCH_BYTES),
            table: TableConfig::from_budget(
                params.table_budget / n_writers,
                params.bucket_size,
                acc_width,
            ),
            n_shards,
            spill_buf: (params.spill_budget / (n_writers * n_shards)).max(MIN_SPILL_BUF),
        }
    }

    pub fn log(&self) {
        info!(
            "dedup tables: {} writers x {} buckets x {} entries ({} MiB fixed, {} MiB heap each); \
             spill: {} shards x {} KiB per writer",
            self.n_writers,
            self.table.n_buckets(),
            self.table.bucket_size,
            self.table.fixed_bytes() >> 20,
            self.table.heap_budget >> 20,
            self.n_shards,
            self.spill_buf >> 10
        );
    }
}

/// 80% of the process' file handle limit.
#[cfg(unix)]
fn sys_open_files() -> u32 {
    use nix::sys::resource::{Resource, getrlimit};

    match getrlimit(Resource::RLIMIT_NOFILE) {
        Ok((soft, _)) => (soft * 8 / 10).clamp(64, 8192) as u32,
        Err(_) => 512,
    }
}

#[cfg(windows)]
fn sys_open_files() -> u32 {
    2048
}
