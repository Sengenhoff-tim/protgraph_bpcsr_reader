//! Reads protein graphs produced by ProtGraph (`.bpcsr`), finds all peptides within
//! the queried mass intervals and writes them deduplicated.
pub mod convert_graphs;
pub mod deduplicate_output;
pub mod parameters;
pub mod pipeline;
pub mod process_graphs;
pub mod shared;

#[cfg(test)]
mod tests;
