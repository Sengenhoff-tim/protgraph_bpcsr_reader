pub mod dedup_compressor;
/// thread spawners for deduplication
pub mod dedup_dispatcher;
pub mod dedup_workers;
pub mod dedup_writer;

pub use dedup_compressor::spawn_compressor;
pub use dedup_dispatcher::spawn_dispatcher;
pub use dedup_workers::spawn_worker;
pub use dedup_writer::spawn_writers;
