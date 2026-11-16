pub mod dedup_compressor;
/// threads of the output stage
pub mod dedup_workers;

pub use dedup_compressor::spawn_compressor;
pub use dedup_workers::shard_worker;
