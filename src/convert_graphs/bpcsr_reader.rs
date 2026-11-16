use std::io::{self, BufRead, Read};

use anyhow::{Context, Result, bail, ensure};
use byteorder::{BigEndian, ReadBytesExt};

use crate::process_graphs::{
    graph::{ProteinGraph, RawEdge, RawNode},
    utilities::{Interval, encode_residue},
};

/// Reads one graph of a .bpcsr file produced by protgraph and lays it out as arena.
/// Only the first accession is kept; positions and iso data are skipped.
pub fn read_single_graph<R: BufRead>(num_acc: u32, reader: &mut R) -> Result<ProteinGraph> {
    // Read counts (big-endian)
    let n_nodes = reader.read_u32::<BigEndian>().context("reading node count")? as usize;
    let n_edges = reader.read_u32::<BigEndian>().context("reading edge count")? as usize;
    let n_pdbs = reader.read_u32::<BigEndian>().context("reading pdb count")? as usize;

    // Accessions (AC): num_acc NUL-terminated strings, only the first is kept
    let mut scratch = Vec::new();
    let mut accession = None;
    for _ in 0..num_acc {
        read_cstring_into(reader, &mut scratch).context("reading accession")?;
        accession.get_or_insert_with(|| scratch.iter().map(|&b| b as char).collect::<String>());
    }
    let accession = accession.unwrap_or_else(|| "unknown".to_string());

    read_graph_body(reader, n_nodes, n_edges, n_pdbs, accession.clone(), &mut scratch)
        .with_context(|| format!("reading graph of accession '{accession}'"))
}

fn read_graph_body<R: BufRead>(
    reader: &mut R,
    n_nodes: usize,
    n_edges: usize,
    n_pdbs: usize,
    accession: String,
    scratch: &mut Vec<u8>,
) -> Result<ProteinGraph> {
    ensure!(n_nodes > 0, "graph has no nodes");

    // Nodes (NO): n_nodes u32 BE, cumulative end offsets into the edge list
    let edge_ends = read_u32_vec(reader, n_nodes).context("reading nodes")?;
    let mut prev = 0;
    for &end in &edge_ends {
        ensure!(
            prev <= end && end as usize <= n_edges,
            "invalid edge offsets: {prev}..{end} of {n_edges} edges"
        );
        prev = end;
    }

    // Edges (ED): n_edges u32 BE, target node index
    let targets = read_u32_vec(reader, n_edges).context("reading edges")?;
    if let Some(&t) = targets.iter().find(|&&t| t as usize >= n_nodes) {
        bail!("edge target {t} out of range for {n_nodes} nodes");
    }

    // Sequences (SQ): n_nodes cstrings, stored as residue codes.
    // Root and terminal carry no residues.
    let mut seq_codes = Vec::new();
    let mut seq_ends = Vec::with_capacity(n_nodes);
    for node in 0..n_nodes {
        read_cstring_into(reader, scratch).context("reading sequences")?;
        if node != 0 && node != n_nodes - 1 {
            for &c in scratch.iter() {
                let Some(code) = encode_residue(c) else {
                    bail!("invalid amino acid {:?} in node {node}", c as char);
                };
                seq_codes.push(code);
            }
        }
        seq_ends.push(seq_codes.len());
    }

    // Position (PO): n_nodes u16, Iso index (IS): n_nodes u8, Iso position (IP): n_nodes u16
    skip_bytes(reader, n_nodes * (2 + 1 + 2)).context("skipping position and iso data")?;

    // Mono weight (MW): n_nodes i64 BE
    let mut mono_weight = vec![0i64; n_nodes];
    reader
        .read_i64_into::<BigEndian>(&mut mono_weight)
        .context("reading mono weights")?;

    // Cleaved (CL): n_edges u8 -> bool
    let cleaved = read_bytes(reader, n_edges).context("reading cleaved")?;

    // Qualifiers (QU): n_edges cstrings, unused
    for _ in 0..n_edges {
        read_cstring_into(reader, scratch).context("reading qualifiers")?;
    }

    // Variant count (VC): n_edges u8
    let ft_count = read_bytes(reader, n_edges).context("reading variant count")?;

    // PDBs: n_nodes * n_pdbs (lower, upper) i64 BE
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

    let nodes: Vec<RawNode> = (0..n_nodes)
        .map(|i| {
            let range = |ends: &[usize]| (if i == 0 { 0 } else { ends[i - 1] })..ends[i];
            let edge_start = if i == 0 { 0 } else { edge_ends[i - 1] as usize };
            RawNode {
                mono_weight: mono_weight[i],
                pdbs: &pdbs[range(&pdb_ends)],
                edges: &edges[edge_start..edge_ends[i] as usize],
                seq: &seq_codes[range(&seq_ends)],
            }
        })
        .collect();

    ProteinGraph::build(accession, &nodes)
}

fn read_bytes<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; count];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_u32_vec<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<u32>> {
    let mut out = vec![0u32; count];
    reader.read_u32_into::<BigEndian>(&mut out)?;
    Ok(out)
}

fn skip_bytes<R: BufRead>(reader: &mut R, count: usize) -> Result<()> {
    let skipped = io::copy(&mut reader.by_ref().take(count as u64), &mut io::sink())?;
    ensure!(skipped == count as u64, "unexpected end of file");
    Ok(())
}

/// Reads a NUL-terminated string into `buf` (without the NUL).
fn read_cstring_into<R: BufRead>(reader: &mut R, buf: &mut Vec<u8>) -> Result<()> {
    buf.clear();
    reader.read_until(0u8, buf)?;
    if matches!(buf.last(), Some(0)) {
        buf.pop();
    }
    Ok(())
}

/// Flat, per node sorted intervals plus cumulative end offsets per node.
fn read_pdbs<R: BufRead>(
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
