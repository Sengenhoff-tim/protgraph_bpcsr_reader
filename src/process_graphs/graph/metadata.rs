use anyhow::Result;

use crate::{
    process_graphs::utilities::{PathState, StringTable, pack_into},
    shared::{Meta},
};

/// Struct containing all data not immediately relevant for traversal
pub struct MetaData {
    pub accessions: Vec<String>,
    pub iso_position: Box<[u16]>,
    pub sequences: StringTable,
}

impl MetaData {
    /// Metas attached to a record. Matches the single meta the old
    /// `add_to_buffer` wrote.
    pub fn build_metas(&self, _trace: &[PathState]) -> Result<Vec<Meta>> {
        Ok(vec![Meta {
            id: self.accessions[0].clone(),
            pos: u16::MAX,
        }])
    }

    pub fn build_sequence(
        &self,
        trace: &[PathState],
        sequence_buffer: &mut Vec<u8>,
    ) -> Result<()> {
        if trace.len() < 2 {
            return Ok(());
        }
        pack_into(
            sequence_buffer,
            trace[1..trace.len() - 1]
                .iter()
                .flat_map(|state| self.sequences.get_slice(state.node as usize).iter()),
        )?;
        Ok(())
    }
}