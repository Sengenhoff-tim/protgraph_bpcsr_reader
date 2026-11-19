//! Long-lived threads of the traversal pipeline:
//! reader → dispatchers → workers → writers (see `process_graphs::orchestrator`).
mod dispatcher;
mod reader;
mod spiller;
mod supervisor;
mod worker;
mod writer;
mod writer_msg;

pub use dispatcher::dispatcher_loop;
pub use reader::reader_loop;
pub use spiller::Spiller;
pub use supervisor::Supervisor;
pub use worker::{FlushRequest, worker_loop};
pub use writer::Writer;
pub use writer_msg::{WriterMsg, WriterReport};
