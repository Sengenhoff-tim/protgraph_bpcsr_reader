//! In-memory dedup of `sequence → accession ids` with bounded memory.
//!
//! The table is a flat array of `2^bits` buckets with `bucket_size` slots each,
//! addressed by the top bits of the sequence hash. A bucket is a small most-recently-used
//! list: a hit appends the accession and moves the entry to the front, a new sequence is
//! inserted at the front. When a bucket is full its last entry is evicted (with all its
//! accessions). Entries whose sequence or accession list outgrew the inline storage
//! count against a separate heap budget; when that is exceeded, such entries are evicted
//! round-robin.
//!
//! Each accession is kept at most once per sequence.
//!
//! Every copy of a sequence lands in the same bucket, so a bucket that never evicted
//! holds complete results. Buckets that evicted are marked; their remaining entries must
//! be merged with the evicted ones later (see [`DedupTable::drain`]).

mod table;
mod table_config;
mod table_entry;
mod table_stats;

pub use table::DedupTable;
pub use table_config::TableConfig;
pub use table_entry::{AccBytes, Entry, SeqBytes};
pub use table_stats::TableStats;
