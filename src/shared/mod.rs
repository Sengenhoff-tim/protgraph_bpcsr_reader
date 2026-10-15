/// bin entry is shared between processing and deduplication
pub mod bin_entry;
pub use bin_entry::EntryBuffer;

/// bin entry is shared between processing and deduplication
pub mod logging;
pub use logging::{init_logging, LoggingConfig};
