use std::io::BufRead;

use anyhow::{Context, Result, anyhow, bail};
use byteorder::{BigEndian, ReadBytesExt};

use crate::process_graphs::{
    graph::{MetaData, ProteinGraph, TraversalData},
    utilities::{Interval, Pdbs, StringTable},
};

/// reads .bpcsr files produced by protgraph
pub fn read_single_graph<R: BufRead>(num_acc: u32, reader: &mut R) -> Result<ProteinGraph> {
    let n_acc = num_acc as usize;
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

    // Accessions (AC): num_acc NUL-terminated strings
    let mut accessions: Vec<String> = Vec::with_capacity(n_acc);
    for _ in 0..n_acc {
        accessions.push(read_cstring(reader).context("reading accession")?);
    }

    let primary_accession = accessions
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    // Nodes (NO): n_nodes u32 BE
    let nodes = read_u32_vec(reader, n_nodes).context(format!(
        "reading nodes for accession '{}'",
        primary_accession
    ))?;

    // Edges (ED): n_edges u32 BE
    let edges = read_u32_vec(reader, n_edges).context(format!(
        "reading edges for accession '{}'",
        primary_accession
    ))?;

    let sequences = build_from_reader(reader, n_nodes).context(format!(
        "reading sequences for accession '{}'",
        primary_accession
    ))?;

    // Position (PO): n_nodes u16 BE
    let position = read_u16_vec(reader, n_nodes).context(format!(
        "reading position for accession '{}'",
        primary_accession
    ))?;

    // Iso index (IS): n_nodes u8
    let iso_index = read_u8_vec(reader, n_nodes).context(format!(
        "reading iso index for accession '{}'",
        primary_accession
    ))?;

    // Iso position (IP): n_nodes u16 BE
    let iso_position = read_u16_vec(reader, n_nodes).context(format!(
        "reading iso position for accession '{}'",
        primary_accession
    ))?;

    // Mono weight (MW): n_nodes i64 BE
    let mut mono_weight = Vec::with_capacity(n_nodes);
    for _ in 0..n_nodes {
        mono_weight.push(reader.read_i64::<BigEndian>().context(format!(
            "reading mono weight for accession '{}'",
            primary_accession
        ))?);
    }

    // Cleaved (CL): n_edges bytes -> bool
    let mut cleaved = vec![0u16; n_edges];
    for item in cleaved.iter_mut() {
        *item = (reader.read_u8().context(format!(
            "reading cleaved for accession '{}'",
            primary_accession
        ))? != 0) as u16;
    }

    let qualifiers = build_from_reader(reader, n_edges).context(format!(
        "reading qualifiers for accession '{}'",
        primary_accession
    ))?;

    // Variant count (VC): n_edges u8
    let ft_count = read_u8_vec(reader, n_edges).context(format!(
        "reading variant count for accession '{}'",
        primary_accession
    ))?;

    // PDBs: n_nodes * n_pdbs
    let pdbs = read_pdbs(reader, n_nodes, n_pdbs).context(format!(
        "reading pdbs for accession '{}'",
        primary_accession
    ))?;

    let ft_clv_edge: Box<[(u16, u16, u32)]> = ft_count
        .iter()
        .zip(cleaved.iter())
        .zip(edges.iter())
        .map(|((&vc, &cl), &ed)| (vc as u16, cl, ed))
        .collect::<Vec<_>>()
        .into_boxed_slice();

    Ok(ProteinGraph {
        traversal_data: TraversalData {
            nodes: nodes.into_boxed_slice(),
            mono_weight: mono_weight.into_boxed_slice(),
            ft_clv_edge,
            pdbs,
        },
        meta_data: MetaData {
            accessions,
            position: position.into_boxed_slice(),
            iso_index: iso_index.into_boxed_slice(),
            iso_position: iso_position.into_boxed_slice(),
            sequences,
            qualifiers,
        },
    })
}

fn build_from_reader<R: BufRead>(reader: &mut R, count: usize) -> Result<StringTable> {
    let mut items = Vec::with_capacity(count);

    for _ in 0..count {
        items.push(read_cstring(reader)?);
    }

    Ok(StringTable::build_from_strings(items))
}

fn read_u8_vec<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<u8>> {
    read_be_vec(reader, count, 1, |b| b[0])
}

fn read_u16_vec<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<u16>> {
    read_be_vec(reader, count, 2, |b| u16::from_be_bytes([b[0], b[1]]))
}

/// check for u32::MAX which is used as sentinel in TraversalData
fn read_u32_vec<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<u32>> {
    let mut out = Vec::with_capacity(count);
    let mut buf = [0u8; 4];

    for i in 0..count {
        reader.read_exact(&mut buf)?;
        let value = u32::from_be_bytes(buf);

        if value == u32::MAX {
            return Err(anyhow!(
                "u32::MAX at index {} (reserved sentinel; possible overflow/truncation)",
                i
            ));
        }

        out.push(value);
    }

    Ok(out)
}

fn read_be_vec<R: BufRead, T>(
    reader: &mut R,
    count: usize,
    byte_len: usize,
    parse: fn(&[u8]) -> T,
) -> Result<Vec<T>> {
    let total = count
        .checked_mul(byte_len)
        .ok_or_else(|| anyhow!("overflow in allocation size"))?;
    let mut buf = vec![0u8; total];
    reader.read_exact(&mut buf)?;

    let mut out = Vec::with_capacity(count);

    for chunk in buf.chunks_exact(byte_len) {
        out.push(parse(chunk));
    }

    Ok(out)
}

fn read_cstring<R: BufRead>(reader: &mut R) -> Result<String> {
    let mut bytes = Vec::new();

    // read up to and including NUL
    reader.read_until(0u8, &mut bytes)?;

    // drop terminating NUL
    if matches!(bytes.last(), Some(0)) {
        bytes.pop();
    }

    Ok(bytes.into_iter().map(|b| b as char).collect())
}

fn read_pdbs<R: BufRead>(reader: &mut R, n_nodes: usize, n_pdbs: usize) -> Result<Pdbs> {
    let mut node_lists: Vec<Vec<Interval>> = Vec::with_capacity(n_nodes);

    for _node in 0..n_nodes {
        let mut node_vec = Vec::new();

        for _slot in 0..n_pdbs {
            let lower = reader.read_i64::<BigEndian>()?;
            let upper = reader.read_i64::<BigEndian>()?;

            // skip sentinel
            if lower == i64::MAX {
                continue;
            }

            if upper != i64::MAX && upper < lower {
                bail!(format!(
                    "Malformed input: upper:{} < lower:{}",
                    upper, lower
                ));
            }

            node_vec.push(Interval { lower, upper });
        }

        // sorted for early exit in traversal
        node_vec.sort_unstable_by_key(|iv| iv.lower);

        node_lists.push(node_vec);
    }

    Pdbs::from_node_lists(node_lists)
}
