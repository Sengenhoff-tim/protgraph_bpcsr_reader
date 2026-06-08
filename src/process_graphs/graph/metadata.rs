use anyhow::Result;

use crate::process_graphs::utilities::StringTable;

/// Struct containing all data not immediately relevant for traversal
pub struct MetaData {
    pub accessions: Vec<String>,
    pub position: Box<[u16]>,
    pub iso_position: Box<[u16]>,
    pub iso_index: Box<[u8]>,
    pub cleaved: Vec<bool>,
    pub sequences: StringTable,
    pub qualifiers: StringTable,
}

/// helper struct
struct ForwardPass {
    iso_idx: u8,
    mssclvg: u32,
    spos: u16,
    last_node_idx: Option<(usize, usize)>,
}

// Buffer layout (little-endian):
// - u32 record size
// - u32 sequence length + sequence bytes
// - u32 accession length + accession bytes
// - u32 qualifiers length + qualifiers bytes
// - u16 start position; u16::MAX = None
// - u16 end position; u16::MAX = None
// - u32 miss-cleavage count

impl MetaData {
    /// builds entry from trace
    pub fn build_peptide(
        &self,
        trace: &[(u32, Option<u32>)],
        entry_buffer: &mut Vec<u8>,
        qualifier_buffer: &mut String,
    ) -> Result<()> {
        let trace_len = trace.len();

        if trace_len < 2 {
            return Ok(());
        }

        let record_start = entry_buffer.len();

        entry_buffer.extend_from_slice(&[0u8; 4]);

        let fwd_res = self.forward_pass(trace, entry_buffer, qualifier_buffer, trace_len)?;

        let acc = self
            .accessions
            .get(fwd_res.iso_idx as usize)
            .map(|s| s.as_str())
            .unwrap_or("NOT FOUND");

        entry_buffer.extend_from_slice(&(acc.len() as u32).to_le_bytes());
        entry_buffer.extend_from_slice(acc.as_bytes());

        // final edge
        if let Some(last) = trace.last()
            && let Some(edge) = last.1
        {
            let q = self.qualifiers.get_str(edge as usize);

            if !q.is_empty() {
                qualifier_buffer.push_str(q);
                qualifier_buffer.push(',');
            }
        }

        let qualifiers_str = qualifier_buffer
            .strip_suffix(',')
            .unwrap_or(qualifier_buffer);

        entry_buffer.extend_from_slice(&(qualifiers_str.len() as u32).to_le_bytes());
        entry_buffer.extend_from_slice(qualifiers_str.as_bytes());

        let mut epos = u16::MAX;

        if let Some((node_idx, seq_len)) = fwd_res.last_node_idx {
            let iso_pos = self.iso_position[node_idx];

            if iso_pos != u16::MAX {
                epos = iso_pos + seq_len as u16 - 1
            } else {
                let pos = self.position[node_idx];
                if pos != u16::MAX {
                    epos = pos + seq_len as u16 - 1
                }
            };
        }

        entry_buffer.extend_from_slice(&fwd_res.spos.to_le_bytes());
        entry_buffer.extend_from_slice(&epos.to_le_bytes());
        entry_buffer.extend_from_slice(&fwd_res.mssclvg.to_le_bytes());

        let record_len = (entry_buffer.len() - record_start - 4) as u32;

        entry_buffer[record_start..record_start + 4].copy_from_slice(&record_len.to_le_bytes());

        Ok(())
    }

    /// helper function, builds seq and collects mssclvg, idx for accession
    fn forward_pass(
        &self,
        trace: &[(u32, Option<u32>)],
        entry_buffer: &mut Vec<u8>,
        qualifier_buffer: &mut String,
        trace_len: usize,
    ) -> Result<ForwardPass> {
        let seq_len_pos = entry_buffer.len();

        entry_buffer.extend_from_slice(&[0u8; 4]);

        let mut iso_idx: u8 = 0;
        let mut mssclvg: u32 = 0;

        let mut last_seq_node: Option<(usize, usize)> = None;

        let mut spos_retrieved = false;

        let mut spos = u16::MAX;

        for &(node, edge) in &trace[1..trace_len - 1] {
            let node_idx = node as usize;

            let seq = self.sequences.get_slice(node_idx);

            entry_buffer.extend_from_slice(seq);

            if !spos_retrieved {
                spos_retrieved = true;

                let iso_pos = self.iso_position[node_idx];

                if iso_pos != u16::MAX {
                    spos = iso_pos;
                } else {
                    let pos = self.position[node_idx];
                    if pos != u16::MAX {
                        spos = pos;
                    }
                }
            }

            last_seq_node = Some((node_idx, seq.len()));

            iso_idx = iso_idx.max(self.iso_index[node_idx]);

            if let Some(e) = edge {
                if self.cleaved[e as usize] {
                    mssclvg += 1;
                }

                let q = self.qualifiers.get_str(e as usize);
                if !q.is_empty() {
                    qualifier_buffer.push_str(q);
                    qualifier_buffer.push(',');
                }
            }
        }

        let seq_len = (entry_buffer.len() - seq_len_pos - 4) as u32;

        entry_buffer[seq_len_pos..seq_len_pos + 4].copy_from_slice(&seq_len.to_le_bytes());

        Ok(ForwardPass {
            iso_idx,
            mssclvg,
            spos,
            last_node_idx: last_seq_node,
        })
    }
}
