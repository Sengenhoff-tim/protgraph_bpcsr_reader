/// Sizing and limits of the traversal pipeline.
#[derive(Debug, Clone)]
pub struct ProcessParams {
    pub threads: usize,
    pub max_vars: u16,
    pub max_cleavages: u16,
    /// entries per dedup bucket
    pub bucket_size: usize,
    /// memory for all dedup tables together
    pub table_budget: usize,
    /// memory for worker → writer batches
    pub batch_budget: usize,
    /// memory for spill buffers of all writers
    pub spill_budget: usize,
    /// open spill files over all writers; defaults to 80% of the process limit
    pub max_handles: Option<u32>,
    /// frontier jobs per graph
    pub jobs_per_graph: usize,
}
