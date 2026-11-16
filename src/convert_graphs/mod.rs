//! Conversion stage: reads the .bpcsr produced by protgraph once and writes the
//! graphs in arena layout (`.pgarena`) together with the global accession table.
//! Later passes read graphs from it with a bulk copy per graph.
mod accession_table;
mod binary_read;
pub(crate) mod bpcsr_reader;
mod bpcsr_sections;
mod graph_conversion;

pub use accession_table::AccessionTable;
pub use graph_conversion::{convert_graphs, prepare_graphs};
