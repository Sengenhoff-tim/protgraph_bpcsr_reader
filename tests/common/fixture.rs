//! Test fixtures independent of the production code: a plain in-memory model of a
//! .bpcsr graph, a writer and parser for the file format, a random graph generator and
//! a naive reference traversal.
//!
//! Shared by the unit tests (included via `#[path]` from `src/tests/graph/mod.rs`) and the
//! integration tests; the including module provides `Interval`.
#![allow(dead_code)]

use rand::{prelude::*, rngs::StdRng};

use super::Interval;

pub const PDB_SENTINEL: (i64, i64) = (i64::MAX, i64::MAX);

/// Mirrors the .bpcsr sections one to one.
#[derive(Clone, Debug, Default)]
pub struct TestGraph {
    pub accessions: Vec<String>,
    /// cumulative edge end offset per node (NO)
    pub edge_ends: Vec<u32>,
    /// target node per edge (ED)
    pub targets: Vec<u32>,
    pub seqs: Vec<String>,
    pub weights: Vec<i64>,
    pub cleaved: Vec<u8>,
    pub qualifiers: Vec<String>,
    pub vc: Vec<u8>,
    pub n_pdbs: usize,
    /// exactly `n_pdbs` slots per node, unused slots are `PDB_SENTINEL`
    pub pdbs: Vec<Vec<(i64, i64)>>,
}

impl TestGraph {
    pub fn n_nodes(&self) -> usize {
        self.seqs.len()
    }

    pub fn edges_of(&self, node: usize) -> std::ops::Range<usize> {
        let start = if node == 0 {
            0
        } else {
            self.edge_ends[node - 1] as usize
        };
        start..self.edge_ends[node] as usize
    }
}

/// Serializes graphs into the .bpcsr format, including the leading accession count.
pub fn write_bpcsr(graphs: &[TestGraph]) -> Vec<u8> {
    let mut out = Vec::new();
    let cstr = |out: &mut Vec<u8>, s: &str| {
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    };
    for g in graphs {
        let n = g.n_nodes();
        out.extend((g.accessions.len() as u32).to_be_bytes());
        out.extend((n as u32).to_be_bytes());
        out.extend((g.targets.len() as u32).to_be_bytes());
        out.extend((g.n_pdbs as u32).to_be_bytes());
        for a in &g.accessions {
            cstr(&mut out, a);
        }
        g.edge_ends.iter().for_each(|v| out.extend(v.to_be_bytes()));
        g.targets.iter().for_each(|v| out.extend(v.to_be_bytes()));
        g.seqs.iter().for_each(|s| cstr(&mut out, s));
        (0..n).for_each(|i| out.extend((i as u16).to_be_bytes())); // position
        (0..n).for_each(|_| out.push(0)); // iso index
        (0..n).for_each(|i| out.extend((i as u16 + 7).to_be_bytes())); // iso position
        g.weights.iter().for_each(|v| out.extend(v.to_be_bytes()));
        out.extend(&g.cleaved);
        g.qualifiers.iter().for_each(|s| cstr(&mut out, s));
        out.extend(&g.vc);
        for node in &g.pdbs {
            assert_eq!(node.len(), g.n_pdbs);
            for &(lo, hi) in node {
                out.extend(lo.to_be_bytes());
                out.extend(hi.to_be_bytes());
            }
        }
    }
    out
}

struct ByteCursor<'a> {
    data: &'a [u8],
    p: usize,
}

impl ByteCursor<'_> {
    fn take(&mut self, n: usize) -> &[u8] {
        let s = &self.data[self.p..self.p + n];
        self.p += n;
        s
    }
    fn u32(&mut self) -> u32 {
        u32::from_be_bytes(self.take(4).try_into().unwrap())
    }
    fn i64(&mut self) -> i64 {
        i64::from_be_bytes(self.take(8).try_into().unwrap())
    }
    fn cstr(&mut self) -> String {
        let len = self.data[self.p..].iter().position(|&b| b == 0).unwrap();
        let s = String::from_utf8(self.take(len).to_vec()).unwrap();
        self.p += 1;
        s
    }
}

/// Parses a whole .bpcsr file.
pub fn parse_bpcsr(data: &[u8]) -> Vec<TestGraph> {
    let mut c = ByteCursor { data, p: 0 };
    let mut graphs = Vec::new();
    while c.p < data.len() {
        let n_acc = c.u32() as usize;
        let n = c.u32() as usize;
        let e = c.u32() as usize;
        let n_pdbs = c.u32() as usize;
        let mut g = TestGraph {
            accessions: (0..n_acc).map(|_| c.cstr()).collect(),
            n_pdbs,
            ..Default::default()
        };
        g.edge_ends = (0..n).map(|_| c.u32()).collect();
        g.targets = (0..e).map(|_| c.u32()).collect();
        g.seqs = (0..n).map(|_| c.cstr()).collect();
        c.take(n * 5);
        g.weights = (0..n).map(|_| c.i64()).collect();
        g.cleaved = c.take(e).to_vec();
        g.qualifiers = (0..e).map(|_| c.cstr()).collect();
        g.vc = c.take(e).to_vec();
        g.pdbs = (0..n)
            .map(|_| (0..n_pdbs).map(|_| (c.i64(), c.i64())).collect())
            .collect();
        graphs.push(g);
    }
    graphs
}

/// Random DAG in topological order: node 0 is the start, the last node the end.
/// Every node i links to i+1, plus random forward edges.
pub fn random_graph(rng: &mut StdRng, n_nodes: usize, n_pdbs: usize) -> TestGraph {
    const AA: &[u8] = b"ACDEFGHIKLMNPQRSTVWY";
    let mut g = TestGraph {
        accessions: vec![format!("ACC{}", rng.random_range(0..1000)), "SECOND".into()],
        n_pdbs,
        ..Default::default()
    };

    for i in 0..n_nodes {
        let seq = match i {
            0 => "__start__".to_string(),
            _ if i == n_nodes - 1 => "__end__".to_string(),
            _ => (0..rng.random_range(1..=10))
                .map(|_| AA[rng.random_range(0..AA.len())] as char)
                .collect(),
        };
        g.seqs.push(seq);
        g.weights.push(rng.random_range(0..100));

        if i + 1 < n_nodes {
            let mut targets = vec![i as u32 + 1];
            for j in i + 2..n_nodes.min(i + 5) {
                if rng.random_bool(0.4) {
                    targets.push(j as u32);
                }
            }
            for t in targets {
                g.targets.push(t);
                g.cleaved.push(rng.random_bool(0.3) as u8);
                g.vc.push(if rng.random_bool(0.2) {
                    rng.random_range(1..=3)
                } else {
                    0
                });
                g.qualifiers.push("q".into());
            }
        }
        g.edge_ends.push(g.targets.len() as u32);

        let mut slots: Vec<(i64, i64)> = (0..n_pdbs)
            .map(|_| match rng.random_range(0..4) {
                0 => PDB_SENTINEL,
                1 => (rng.random_range(0..600), i64::MAX),
                _ => {
                    let lo = rng.random_range(0..600);
                    (lo, lo + rng.random_range(0..200))
                }
            })
            .collect();
        slots.shuffle(rng);
        g.pdbs.push(slots);
    }
    g
}

/// Naive traversal straight from the file model: enumerates every root→end path within
/// the limits where, after each step, some node interval overlaps some query interval
/// shifted by the weight so far. Returns the sequences (without start/end node).
pub fn reference_peptides(
    g: &TestGraph,
    queries: &[Interval],
    max_vars: u16,
    max_cleavages: u16,
) -> Vec<String> {
    fn passes(g: &TestGraph, node: usize, tv: i64, queries: &[Interval]) -> bool {
        g.pdbs[node].iter().any(|&(lo, hi)| {
            lo != i64::MAX
                && queries
                    .iter()
                    .any(|q| lo <= q.upper - tv && hi >= q.lower - tv)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn dfs(
        g: &TestGraph,
        queries: &[Interval],
        limits: (u16, u16),
        node: usize,
        var: u16,
        clv: u16,
        tv: i64,
        path: &mut Vec<usize>,
        out: &mut Vec<String>,
    ) {
        if node == g.n_nodes() - 1 {
            out.push(
                path[1..path.len() - 1]
                    .iter()
                    .map(|&n| g.seqs[n].as_str())
                    .collect(),
            );
            return;
        }
        for e in g.edges_of(node) {
            let t = g.targets[e] as usize;
            let var = var + g.vc[e] as u16;
            let clv = clv + (g.cleaved[e] != 0) as u16;
            let tv = tv + g.weights[t];
            if var > limits.0 || clv > limits.1 || !passes(g, t, tv, queries) {
                continue;
            }
            path.push(t);
            dfs(g, queries, limits, t, var, clv, tv, path, out);
            path.pop();
        }
    }

    let mut out = Vec::new();
    dfs(
        g,
        queries,
        (max_vars, max_cleavages),
        0,
        0,
        0,
        0,
        &mut vec![0],
        &mut out,
    );
    out.sort();
    out
}
