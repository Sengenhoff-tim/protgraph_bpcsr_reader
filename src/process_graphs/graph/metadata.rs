use anyhow::Result;

use crate::process_graphs::utilities::{PathState, StringTable, pack_into};

/// Struct containing all data not immediately relevant for traversal
pub struct MetaData {
    pub accessions: Vec<String>,
    pub position: Box<[u16]>,
    pub iso_position: Box<[u16]>,
    pub iso_index: Box<[u8]>,
    pub sequences: StringTable,
    pub qualifiers: StringTable,
}

const ID_LEN: usize = 10;
const SIZE_LEN: usize = 4;
const START_LEN: usize = 2;

impl MetaData {
    //  record layout (little-endian):
    // - u32 record size
    // - 10*char bytes id 
    // - u16 start position
    // - 5-bit packed sequence, 0-padded
    pub fn build_peptide(
        &self,
        trace: &[PathState],
        sequence_buffer: &mut Vec<u8>,
        entry_buffer: &mut Vec<u8>,
    ) -> Result<()> {
        if trace.len() < 2 {
            return Ok(());
        }

        let interior = &trace[1..trace.len() - 1];

        pack_into(
            sequence_buffer,
            interior
                .iter()
                .flat_map(|state| self.sequences.get_slice(state.node as usize).iter()),
        )?;

        let mut id = [0u8; ID_LEN];
        id[..self.accessions[0].len()].copy_from_slice(self.accessions[0].as_bytes());

        let record_size = u32::try_from(ID_LEN + START_LEN + sequence_buffer.len())?;

        entry_buffer.reserve(record_size as usize);
        entry_buffer.extend_from_slice(&record_size.to_le_bytes());
        entry_buffer.extend_from_slice(&id);
        entry_buffer.extend_from_slice(
            &self.get_start_position(trace[1].node as usize).to_le_bytes(),
        );
        entry_buffer.extend_from_slice(sequence_buffer);

        Ok(())
    }

    fn get_start_position(&self, node_idx: usize) -> u16 {
        self.iso_position[node_idx]
            .try_into()
            .or_else(|_| self.position[node_idx].try_into())
            .unwrap_or(u16::MAX)
    }
}