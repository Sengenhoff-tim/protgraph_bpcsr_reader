/// building blocks shared between graph processing and output
pub mod acc_id;
pub mod dedup_table;
pub mod disk_buffer;
pub mod mem_buffer;
pub mod output_batch;

pub use acc_id::{acc_width, decode_accs, dedup_accs, encode_acc};
pub use dedup_table::{DedupTable, Entry, TableConfig, TableStats};
pub use disk_buffer::{DiskBuffer, DiskRecord, write_record};
pub use mem_buffer::{MemBuffer, MemRecord};
pub use output_batch::OutputBatch;

pub mod logging;
pub use logging::{LoggingConfig, init_logging};
