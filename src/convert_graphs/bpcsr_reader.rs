//! Reads graphs from the .bpcsr files produced by protgraph.

use std::io::BufRead;

use anyhow::{Context, Result, ensure};
use byteorder::{BigEndian, ReadBytesExt};

use super::{
    AccessionTable,
    binary_read::{read_bytes, read_cstring_into, read_i64_vec, skip_bytes},
    bpcsr_sections::{read_edge_ends, read_pdbs, read_sequences, read_targets},
};
use crate::process_graphs::graph::{ProteinGraph, RawEdge, RawNode};

/// Reads one graph of a .bpcsr file produced by protgraph and lays it out as arena.
/// Only the first accession is kept (interned into `accessions`); positions and iso
/// data are skipped.
pub fn read_single_graph<R: BufRead>(
    num_acc: u32,
    reader: &mut R,
    accessions: &mut AccessionTable,
) -> Result<ProteinGraph> {
    // Read counts (big-endian)
    let n_nodes = reader
        .read_u32::<BigEndian>()
        .context("reading node count")? as usize;
    let n_edges = reader
        .read_u32::<BigEndian>()
        .context("reading edge count")? as usize;
    let n_pdbs = reader
        .read_u32::<BigEndian>()
        .context("reading pdb count")? as usize;

    // Accessions (AC): num_acc NUL-terminated strings, only the first is kept
    let mut scratch = Vec::new();
    let mut accession = None;
    for _ in 0..num_acc {
        read_cstring_into(reader, &mut scratch).context("reading accession")?;
        accession.get_or_insert_with(|| scratch.iter().map(|&b| b as char).collect::<String>());
    }
    let accession = accession.unwrap_or_else(|| "unknown".to_string());

    let acc_id = accessions.intern(accession.clone())?;
    read_graph_body(reader, n_nodes, n_edges, n_pdbs, acc_id, &mut scratch)
        .with_context(|| format!("reading graph of accession '{accession}'"))
}

/// The sections after the accessions, in file order.
fn read_graph_body<R: BufRead>(
    reader: &mut R,
    n_nodes: usize,
    n_edges: usize,
    n_pdbs: usize,
    acc_id: u32,
    scratch: &mut Vec<u8>,
) -> Result<ProteinGraph> {
    ensure!(n_nodes > 0, "graph has no nodes");

    let edge_ends = read_edge_ends(reader, n_nodes, n_edges)?;
    let targets = read_targets(reader, n_edges, n_nodes)?;
    let (seq_codes, seq_ends) = read_sequences(reader, n_nodes, scratch)?;
    // Position (PO): u16, Iso index (IS): u8, Iso position (IP): u16 per node, unused
    skip_bytes(reader, n_nodes * (2 + 1 + 2)).context("skipping position and iso data")?;
    let mono_weight = read_i64_vec(reader, n_nodes).context("reading mono weights")?;
    let cleaved = read_bytes(reader, n_edges).context("reading cleaved")?;
    // Qualifiers (QU): one string per edge, unused
    for _ in 0..n_edges {
        read_cstring_into(reader, scratch).context("reading qualifiers")?;
    }
    let ft_count = read_bytes(reader, n_edges).context("reading variant count")?;
    let (pdbs, pdb_ends) = read_pdbs(reader, n_nodes, n_pdbs).context("reading pdbs")?;

    let edges: Vec<RawEdge> = targets
        .iter()
        .zip(&ft_count)
        .zip(&cleaved)
        .map(|((&target, &features), &cl)| RawEdge {
            target,
            features,
            cleave: cl != 0,
        })
        .collect();

    let range = |ends: &[usize], i: usize| (if i == 0 { 0 } else { ends[i - 1] })..ends[i];
    let edge_ends: Vec<usize> = edge_ends.iter().map(|&e| e as usize).collect();
    let nodes: Vec<RawNode> = (0..n_nodes)
        .map(|i| RawNode {
            mono_weight: mono_weight[i],
            pdbs: &pdbs[range(&pdb_ends, i)],
            edges: &edges[range(&edge_ends, i)],
            seq: &seq_codes[range(&seq_ends, i)],
        })
        .collect();

    ProteinGraph::build(acc_id, &nodes)
}
