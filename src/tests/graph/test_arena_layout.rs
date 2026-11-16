//! Layout of the node block arena: accessors, word boundaries and limits.

use crate::process_graphs::{
    graph::{ProteinGraph, RawEdge, RawNode},
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
        Interval { lower: -5, upper: 3 },
        Interval {
            lower: 10,
            upper: i64::MAX,
        },
    ];
    let root_edges = [edge(1, 0, false), edge(2, 7, true), edge(3, 3, false)];
    let mid_edges = [edge(3, 1, true)];
    let nodes = [
        RawNode { mono_weight: 0, pdbs: &[], edges: &root_edges, seq: &[] },
        RawNode { mono_weight: -42, pdbs: &pdbs, edges: &mid_edges, seq: &seq },
        RawNode { mono_weight: i64::MAX, pdbs: &[], edges: &[edge(3, 0, false)], seq: &[] },
        RawNode { mono_weight: 1, pdbs: &pdbs[..1], edges: &[], seq: &[] },
    ];

    let g = ProteinGraph::build("P12345".into(), &nodes).unwrap();
    assert_eq!(g.accession(), "P12345");

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
    assert_eq!(n1.pdbs().collect::<Vec<_>>(), pdbs.map(|i| (i.lower, i.upper)));
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
    assert!(ProteinGraph::build("x".into(), &empty).is_err());

    let too_many_features = [edge(1, 8, false)];
    let out_of_range = [edge(5, 0, false)];
    for edges in [&too_many_features[..], &out_of_range[..]] {
        let nodes = [
            RawNode { mono_weight: 0, pdbs: &[], edges, seq: &[] },
            RawNode { mono_weight: 0, pdbs: &[], edges: &[], seq: &[] },
        ];
        assert!(ProteinGraph::build("x".into(), &nodes).is_err());
    }
}

#[test]
fn single_node_graph_is_root_and_terminal() {
    let nodes = [RawNode { mono_weight: 0, pdbs: &[], edges: &[], seq: &[] }];
    let g = ProteinGraph::build("x".into(), &nodes).unwrap();
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
