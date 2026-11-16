//! Turning completed paths into record contents.

use crate::process_graphs::utilities::{PathState, pack_codes_into};

use super::ProteinGraph;

impl ProteinGraph {
    /// Packed sequence of the path, excluding root and terminal.
    pub fn build_sequence(&self, trace: &[PathState], sequence_buffer: &mut Vec<u8>) {
        if trace.len() < 2 {
            sequence_buffer.clear();
            return;
        }
        pack_codes_into(
            sequence_buffer,
            trace[1..trace.len() - 1]
                .iter()
                .flat_map(|state| self.node(state.node).seq()),
        );
    }
}
