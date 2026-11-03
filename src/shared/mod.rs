/// bin entry is shared between processing and deduplication
//pub mod bin_entry;
//pub use bin_entry::{DiskBuffer, MemBuffer, COUNT_LEN, HASH_LEN, SIZE_LEN} ;

pub mod disk_buffer;
pub mod mem_buffer;

pub use disk_buffer::{DiskBuffer, DiskRecord};
pub use mem_buffer::{MemBuffer, Meta, Record};

/// bin entry is shared between processing and deduplication
pub mod logging;
pub use logging::{LoggingConfig, init_logging};
