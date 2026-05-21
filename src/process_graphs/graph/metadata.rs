use anyhow::Result;

use crate::process_graphs::utilities::StringTable;
use crate::shared::{BinEntry, BinEntryMeta};

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
    seq: String,
    qualifiers: String,
    iso_idx: u8,
    mssclvg: u32,
    spos: Option<u16>,
    last_node_idx: Option<(usize, usize)>,
}

impl MetaData {
    /// builds entry from trace
    pub fn build_peptide(&self, trace: &[(u32, Option<u32>)]) -> Result<Option<BinEntry>> {
        let trace_len = trace.len();

        if trace_len < 2 {
            return Ok(None);
        }
        let mut fwd_res = self.forward_pass(trace, trace_len)?;

        // final edge
        if let Some(last) = trace.last()
            && let Some(edge) = last.1
        {
            let q = self.qualifiers.get_str(edge as usize);

            if !q.is_empty() {
                fwd_res.qualifiers.push_str(q);
                fwd_res.qualifiers.push(',');
            }
        }

        let mut epos: Option<u16> = None;

        if let Some((node_idx, seq_len)) = fwd_res.last_node_idx {
            let iso_pos = self.iso_position[node_idx];

            if iso_pos != u16::MAX {
                epos = Some(iso_pos + seq_len as u16 - 1)
            } else {
                let pos = self.position[node_idx];
                if pos != u16::MAX {
                    epos = Some(pos + seq_len as u16 - 1)
                }
            };
        }

        let acc = self
            .accessions
            .get(fwd_res.iso_idx as usize)
            .map(|s| s.as_str())
            .unwrap_or("NOT FOUND");

        let qualifiers_str = fwd_res
            .qualifiers
            .strip_suffix(',')
            .unwrap_or(&fwd_res.qualifiers);

        Ok(Some(BinEntry {
            seq: fwd_res.seq,
            meta: BinEntryMeta {
                acc: acc.to_string(),
                qualifiers: qualifiers_str.to_string(),
                spos: fwd_res.spos,
                epos,
                mssclvg: fwd_res.mssclvg,
            },
        }))
    }

    /// helper function, builds seq and collects mssclvg, idx for accession
    fn forward_pass(&self, trace: &[(u32, Option<u32>)], trace_len: usize) -> Result<ForwardPass> {
        let mut seq_out = String::new();
        let mut qualifiers_out = String::new();
        let mut iso_idx: u8 = 0;
        let mut mssclvg: u32 = 0;

        let mut last_seq_node: Option<(usize, usize)> = None;

        let mut spos_retrieved = false;

        let mut spos = None;

        for &(node, edge) in &trace[1..trace_len - 1] {
            let node_idx = node as usize;

            let seq = self.sequences.get_str(node_idx).to_string();

            if seq.is_empty() {
                continue;
            }
            let seq_len = seq.len();

            seq_out.push_str(&seq);

            if !spos_retrieved {
                spos_retrieved = true;

                let iso_pos = self.iso_position[node_idx];

                if iso_pos != u16::MAX {
                    spos = Some(iso_pos);
                } else {
                    let pos = self.position[node_idx];
                    if pos != u16::MAX {
                        spos = Some(pos);
                    }
                }
            }

            last_seq_node = Some((node_idx, seq_len));

            iso_idx = iso_idx.max(self.iso_index[node_idx]);

            if let Some(e) = edge {
                if self.cleaved[e as usize] {
                    mssclvg += 1;
                }

                let q = self.qualifiers.get_str(e as usize);
                if !q.is_empty() {
                    qualifiers_out.push_str(q);
                    qualifiers_out.push(',');
                }
            }
        }

        Ok(ForwardPass {
            seq: seq_out,
            qualifiers: qualifiers_out,
            iso_idx,
            mssclvg,
            spos,
            last_node_idx: last_seq_node,
        })
    }
}
