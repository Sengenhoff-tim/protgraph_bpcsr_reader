//! Protein graph stored as a node block arena.
mod builder;
mod edge;
mod frontier;
mod node;
mod output;
mod protein_graph;
mod traversal;

pub use builder::RawNode;
#[cfg(test)]
pub use edge::FEATURE_CAP;
pub use edge::{Edge, MAX_VARIANTS, RawEdge};
pub use node::Node;
pub use protein_graph::ProteinGraph;
