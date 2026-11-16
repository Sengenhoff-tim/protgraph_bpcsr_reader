//! Layout of the node block arena: accessors, word boundaries and limits.

use crate::process_graphs::{
    graph::{FEATURE_CAP, MAX_VARIANTS, ProteinGraph, RawEdge, RawNode},
    utilities::{Interval, encode_residue, pack_codes_into, unpack_into},
};

fn codes(s: &str) -> Vec<u8> {
    s.bytes().map(|c| encode_residue(c).unwrap()).collect()
}

fn edge(target: u32, features: u8, cleave: bool) -> RawEdge {
    RawEdge {
        target,
        features,
        cleave,
    }
}

#[test]
fn node_accessors_roundtrip() {
    // odd edge count and a sequence spanning two words exercise the packing boundaries
    let seq = codes("ACDEFGHIK");
    let pdbs = [
        Interval {
            lower: -5,
            upper: 3,
        },
        Interval {
            lower: 10,
            upper: i64::MAX,
        },
    ];
    let root_edges = [edge(1, 0, false), edge(2, 7, true), edge(3, 3, false)];
    let mid_edges = [edge(3, 1, true)];
    let nodes = [
        RawNode {
            mono_weight: 0,
            pdbs: &[],
            edges: &root_edges,
            seq: &[],
        },
        RawNode {
            mono_weight: -42,
            pdbs: &pdbs,
            edges: &mid_edges,
            seq: &seq,
        },
        RawNode {
            mono_weight: i64::MAX,
            pdbs: &[],
            edges: &[edge(3, 0, false)],
            seq: &[],
        },
        RawNode {
            mono_weight: 1,
            pdbs: &pdbs[..1],
            edges: &[],
            seq: &[],
        },
    ];

    let g = ProteinGraph::build(7, &nodes).unwrap();
    assert_eq!(g.acc_id(), 7);

    let root = g.node(0);
    let edges: Vec<_> = root.edges().collect();
    assert_eq!(edges.len(), 3);
    let flags: Vec<_> = edges.iter().map(|e| (e.features(), e.cleaves())).collect();
    assert_eq!(flags, [(0, 0), (7, 1), (3, 0)]);

    // targets are offsets of the referenced blocks, all edges into node 3 agree
    let n1 = g.node(edges[0].target());
    let n2 = g.node(edges[1].target());
    assert_eq!(n1.mono_weight(), -42);
    assert_eq!(n2.mono_weight(), i64::MAX);
    assert_eq!(
        n1.pdbs().collect::<Vec<_>>(),
        pdbs.map(|i| (i.lower, i.upper))
    );
    assert_eq!(n1.seq().collect::<Vec<_>>(), seq);

    let to_end: Vec<u32> = [n1, n2]
        .iter()
        .map(|n| n.edges().next().unwrap().target())
        .chain([edges[2].target()])
        .collect();
    assert!(to_end.iter().all(|&t| t == g.terminal()));
    let end = g.node(g.terminal());
    assert_eq!(end.mono_weight(), 1);
    assert_eq!(end.edges().count(), 0);
    assert_eq!(end.pdbs().count(), 1);
}

#[test]
fn rejects_invalid_graphs() {
    let empty: [RawNode; 0] = [];
    assert!(ProteinGraph::build(0, &empty).is_err());

    let out_of_range = [edge(5, 0, false)];
    let nodes = [
        RawNode {
            mono_weight: 0,
            pdbs: &[],
            edges: &out_of_range,
            seq: &[],
        },
        RawNode {
            mono_weight: 0,
            pdbs: &[],
            edges: &[],
            seq: &[],
        },
    ];
    assert!(ProteinGraph::build(0, &nodes).is_err());
}

#[test]
fn feature_counts_are_capped() {
    let edges = [6, 7, 8, 200].map(|f| edge(1, f, true));
    let nodes = [
        RawNode {
            mono_weight: 0,
            pdbs: &[],
            edges: &edges,
            seq: &[],
        },
        RawNode {
            mono_weight: 0,
            pdbs: &[],
            edges: &[],
            seq: &[],
        },
    ];
    let g = ProteinGraph::build(0, &nodes).unwrap();
    let stored: Vec<_> = g
        .node(0)
        .edges()
        .map(|e| (e.features(), e.cleaves()))
        .collect();
    let cap = FEATURE_CAP as u16;
    assert_eq!(stored, [(6, 1), (cap, 1), (cap, 1), (cap, 1)]);
    assert!(
        MAX_VARIANTS < cap,
        "a capped edge must exceed every allowed max_vars"
    );
}

#[test]
fn root_with_more_than_u16_edges() {
    // protgraph roots link to every peptide start; large variant-rich proteins exceed 65535
    let n = 70_000u32;
    let root_edges: Vec<_> = (1..=n)
        .map(|t| edge(t, (t % 3) as u8, t.is_multiple_of(2)))
        .collect();
    let seq = codes("PEPTIDE");
    let mut nodes = vec![RawNode {
        mono_weight: 0,
        pdbs: &[],
        edges: &root_edges,
        seq: &[],
    }];
    let to_end = [edge(n + 1, 0, false)];
    for _ in 1..=n {
        nodes.push(RawNode {
            mono_weight: 1,
            pdbs: &[],
            edges: &to_end,
            seq: &seq,
        });
    }
    nodes.push(RawNode {
        mono_weight: 0,
        pdbs: &[],
        edges: &[],
        seq: &[],
    });

    let g = ProteinGraph::build(0, &nodes).unwrap();
    let root = g.node(0);
    assert_eq!(root.edges().count(), n as usize);
    for (i, e) in root.edges().enumerate() {
        let t = i as u32 + 1;
        assert_eq!(
            (e.features(), e.cleaves()),
            ((t % 3) as u16, (t.is_multiple_of(2)) as u16)
        );
        let target = g.node(e.target());
        assert_eq!(target.seq().collect::<Vec<_>>(), seq);
        assert_eq!(target.edges().next().unwrap().target(), g.terminal());
    }
}

#[test]
fn node_with_more_than_u16_pdbs() {
    let pdbs: Vec<_> = (0..70_000i64)
        .map(|i| Interval {
            lower: 10 * i,
            upper: 10 * i + 5,
        })
        .collect();
    let seq = codes("ACDEFGHIKLMNPQRSTVWY");
    let nodes = [
        RawNode {
            mono_weight: 0,
            pdbs: &[],
            edges: &[edge(1, 0, false)],
            seq: &[],
        },
        RawNode {
            mono_weight: 3,
            pdbs: &pdbs,
            edges: &[edge(2, 1, true)],
            seq: &seq,
        },
        RawNode {
            mono_weight: 0,
            pdbs: &[],
            edges: &[],
            seq: &[],
        },
    ];
    let g = ProteinGraph::build(0, &nodes).unwrap();
    let node = g.node(g.node(0).edges().next().unwrap().target());
    assert_eq!(node.mono_weight(), 3);
    assert_eq!(node.pdbs().count(), 70_000);
    assert_eq!(node.pdbs().last(), Some((699_990, 699_995)));
    assert_eq!(node.seq().collect::<Vec<_>>(), seq);
    assert_eq!(node.edges().next().unwrap().target(), g.terminal());
    // the last interval is found by the overlap check
    let q = [Interval {
        lower: 699_993,
        upper: 699_994,
    }];
    assert!(node.has_overlapping_interval(&q, 0));
}

#[test]
fn single_node_graph_is_root_and_terminal() {
    let nodes = [RawNode {
        mono_weight: 0,
        pdbs: &[],
        edges: &[],
        seq: &[],
    }];
    let g = ProteinGraph::build(0, &nodes).unwrap();
    assert_eq!(g.terminal(), 0);
}

#[test]
fn pack_codes_roundtrip() {
    let mut packed = Vec::new();
    let mut unpacked = Vec::new();
    for s in ["", "A", "ACDEFGHIK", "ACDEFGHIKLMNPQRSTVWYBZXUOJ"] {
        pack_codes_into(&mut packed, codes(s));
        assert_eq!(packed.len(), (s.len() * 5).div_ceil(8));
        unpack_into(&mut unpacked, packed.iter().copied());
        assert_eq!(unpacked, s.as_bytes());
    }
    assert_eq!(encode_residue(b'_'), None);
    assert_eq!(encode_residue(b'a'), None);
}
