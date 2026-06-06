/// output writers
mod writer_wrapper;
mod writers;

pub use writer_wrapper::WriterWrapper;
pub use writers::{write_meta, write_sequences};
