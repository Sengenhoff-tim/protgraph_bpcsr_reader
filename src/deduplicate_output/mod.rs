//! Output stage: the output files and the final dedup of spilled shard files.
mod chunk_buffer;
mod compressed_stream;
mod io;
mod output_files;
mod output_sink;
mod shard_dedup;
mod threading;

pub use output_files::{
    ACCESSIONS_FILE, METADATA_FILE, OUTPUT_GZIP_LEVEL, PEPTIDES_FILE, output_path, write_accessions,
};
pub use output_sink::{OutputSink, OutputStats};
pub use shard_dedup::dedup_spilled;
