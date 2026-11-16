//! Serialized form of converted graphs (`.pgarena`).
//!
//! ```text
//! file  = MAGIC | table_offset: u64 | graph* | accession table
//! graph = acc_id: u32 | terminal: u32 | n_words: u64 | arena words
//! table = n: u32 | (len: u32 | accession bytes) * n        (at table_offset)
//! ```
//! All integers little endian. The arena is stored as-is, so reading a graph is a single
//! bulk copy. The table comes last because conversion only knows it once all graphs are
//! written; the header points to it.
mod arena_file;
mod arena_format;
mod arena_writer;

pub use arena_file::ArenaFile;
pub use arena_format::{EXTENSION, MAGIC, is_arena_file, read_graph, write_graph};
pub use arena_writer::ArenaWriter;
