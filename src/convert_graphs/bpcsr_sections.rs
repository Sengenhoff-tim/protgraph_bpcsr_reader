//! Readers for the sections of one .bpcsr graph that need validation or reshaping.

use std::io::BufRead;

use anyhow::{Context, Result, bail, ensure};
use byteorder::{BigEndian, ReadBytesExt};

use super::binary_read::{read_cstring_into, read_u32_vec};
use crate::process_graphs::utilities::{Interval, encode_residue};

/// Nodes (NO): cumulative end offsets into the edge list, one per node.
pub fn read_edge_ends<R: BufRead>(
    reader: &mut R,
    n_nodes: usize,
    n_edges: usize,
) -> Result<Vec<u32>> {
    let ends = read_u32_vec(reader, n_nodes).context("reading nodes")?;
    let mut prev = 0;
    for &end in &ends {
        ensure!(
            prev <= end && end as usize <= n_edges,
            "invalid edge offsets: {prev}..{end} of {n_edges} edges"
        );
        prev = end;
    }
    Ok(ends)
}

/// Edges (ED): target node index per edge.
pub fn read_targets<R: BufRead>(
    reader: &mut R,
    n_edges: usize,
    n_nodes: usize,
) -> Result<Vec<u32>> {
    let targets = read_u32_vec(reader, n_edges).context("reading edges")?;
    if let Some(&t) = targets.iter().find(|&&t| t as usize >= n_nodes) {
        bail!("edge target {t} out of range for {n_nodes} nodes");
    }
    Ok(targets)
}

/// Sequences (SQ): one string per node, returned as flat residue codes plus cumulative
/// end offsets per node. Root and terminal carry no residues.
pub fn read_sequences<R: BufRead>(
    reader: &mut R,
    n_nodes: usize,
    scratch: &mut Vec<u8>,
) -> Result<(Vec<u8>, Vec<usize>)> {
    let mut codes = Vec::new();
    let mut ends = Vec::with_capacity(n_nodes);
    for node in 0..n_nodes {
        read_cstring_into(reader, scratch).context("reading sequences")?;
        if node != 0 && node != n_nodes - 1 {
            for &c in scratch.iter() {
                let Some(code) = encode_residue(c) else {
                    bail!("invalid amino acid {:?} in node {node}", c as char);
                };
                codes.push(code);
            }
        }
        ends.push(codes.len());
    }
    Ok((codes, ends))
}

/// PDBs: `n_pdbs` (lower, upper) slots per node. Returns the flat, per node sorted
/// intervals without sentinels plus cumulative end offsets per node.
pub fn read_pdbs<R: BufRead>(
    reader: &mut R,
    n_nodes: usize,
    n_pdbs: usize,
) -> Result<(Vec<Interval>, Vec<usize>)> {
    let mut all = Vec::new();
    let mut ends = Vec::with_capacity(n_nodes);

    for _node in 0..n_nodes {
        let start = all.len();
        for _slot in 0..n_pdbs {
            let lower = reader.read_i64::<BigEndian>()?;
            let upper = reader.read_i64::<BigEndian>()?;

            // skip sentinel
            if lower == i64::MAX {
                continue;
            }
            if upper != i64::MAX && upper < lower {
                bail!("Malformed input: upper:{} < lower:{}", upper, lower);
            }
            all.push(Interval { lower, upper });
        }
        // sorted for early exit in traversal
        all[start..].sort_unstable_by_key(|iv| iv.lower);
        ends.push(all.len());
    }

    Ok((all, ends))
}
