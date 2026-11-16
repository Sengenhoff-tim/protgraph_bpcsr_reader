//! Arena traversal compared against the naive reference traversal from `fixture`.

use std::{io::Cursor, path::PathBuf};

use rand::{RngExt, SeedableRng, rngs::StdRng};

use super::fixture::{TestGraph, parse_bpcsr, random_graph, reference_peptides, write_bpcsr};
use crate::{
    convert_graphs::{AccessionTable, bpcsr_reader::read_single_graph},
    process_graphs::{
        graph::ProteinGraph,
        utilities::{
            Interval, IntervalVecExt, PathState, TraversalContext, WEIGHT_FACTOR, unpack_into,
        },
    },
};

fn to_graph(tg: &TestGraph) -> ProteinGraph {
    let bytes = write_bpcsr(std::slice::from_ref(tg));
    read_single_graph(
        tg.accessions.len() as u32,
        &mut Cursor::new(&bytes[4..]),
        &mut AccessionTable::default(),
    )
    .unwrap()
}

fn ctx(queries: Vec<Interval>, max_vars: u16, max_cleavages: u16) -> TraversalContext {
    TraversalContext {
        max_variants: max_vars,
        max_missed_cleavages: max_cleavages,
        intervals: queries.merge_and_sort().unwrap(),
    }
}

/// Collects the sequence of every completed path, decoded back to letters.
fn collect(
    graph: &ProteinGraph,
    ctx: &TraversalContext,
    path: &mut Vec<PathState>,
    out: &mut Vec<String>,
) {
    let mut packed = Vec::new();
    let mut letters = Vec::new();
    graph
        .traverse_depth_first(ctx, path, &mut |trace| {
            graph.build_sequence(trace, &mut packed);
            unpack_into(&mut letters, packed.iter().copied());
            out.push(String::from_utf8(letters.clone()).unwrap());
            Ok(())
        })
        .unwrap();
}

fn arena_peptides(graph: &ProteinGraph, ctx: &TraversalContext) -> Vec<String> {
    let mut out = Vec::new();
    let mut root = vec![PathState {
        node: 0,
        var: 0,
        cleaves: 0,
        tv: 0,
    }];
    collect(graph, ctx, &mut root, &mut out);
    out.sort();
    out
}

/// Same, but split into frontier jobs first as the dispatcher does.
fn arena_peptides_via_frontier(
    graph: ProteinGraph,
    ctx: &TraversalContext,
    jobs: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    for mut path in graph.build_frontier(ctx, jobs) {
        collect(&graph, ctx, &mut path, &mut out);
    }
    out.sort();
    out
}

#[test]
fn traversal_matches_reference_on_random_graphs() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut total = 0;

    for round in 0..200 {
        let (n_nodes, n_pdbs) = (rng.random_range(2..40), rng.random_range(1..5));
        let tg = random_graph(&mut rng, n_nodes, n_pdbs);
        let queries: Vec<Interval> = (0..rng.random_range(1..4))
            .map(|_| {
                let lower = rng.random_range(0..700);
                Interval {
                    lower,
                    upper: lower + rng.random_range(0..150),
                }
            })
            .collect();
        let (max_vars, max_cleavages) = (rng.random_range(0..4), rng.random_range(0..4));
        let ctx = ctx(queries, max_vars, max_cleavages);

        let expected = reference_peptides(&tg, &ctx.intervals, max_vars, max_cleavages);
        let graph = to_graph(&tg);
        assert_eq!(arena_peptides(&graph, &ctx), expected, "round {round}");
        assert_eq!(
            arena_peptides_via_frontier(graph, &ctx, rng.random_range(1..20)),
            expected,
            "round {round} (frontier)"
        );
        total += expected.len();
    }

    // make sure the random setup actually produces matches, not only pruned graphs
    assert!(total > 200, "only {total} peptides over all rounds");
}

#[test]
fn limits_prune_paths() {
    // start -> A -> end, start -> C -> end, the C route carries a variant and a cleavage.
    // pdbs hold the exact remaining weight after each node, which is 0 everywhere.
    let open = vec![(0, 0)];
    let tg = TestGraph {
        accessions: vec!["P1".into()],
        edge_ends: vec![2, 3, 4, 4],
        targets: vec![1, 2, 3, 3],
        seqs: vec!["__start__".into(), "A".into(), "C".into(), "__end__".into()],
        weights: vec![0, 10, 20, 0],
        cleaved: vec![0, 1, 0, 0],
        qualifiers: vec!["".into(); 4],
        vc: vec![0, 1, 0, 0],
        n_pdbs: 1,
        pdbs: vec![open.clone(), open.clone(), open.clone(), open],
    };
    let g = to_graph(&tg);
    let all = vec![Interval {
        lower: 0,
        upper: 100,
    }];

    assert_eq!(arena_peptides(&g, &ctx(all.clone(), 1, 1)), ["A", "C"]);
    assert_eq!(arena_peptides(&g, &ctx(all.clone(), 0, 1)), ["A"]);
    assert_eq!(arena_peptides(&g, &ctx(all, 1, 0)), ["A"]);
    // only the weight of C (20) is inside the query
    assert_eq!(
        arena_peptides(
            &g,
            &ctx(
                vec![Interval {
                    lower: 15,
                    upper: 25
                }],
                1,
                1
            )
        ),
        ["C"]
    );
}

/// Regression check on real protgraph output. Set `BPCSR_TEST_FILES` to a colon separated
/// list of .bpcsr files and run with `cargo test -- --ignored`.
#[test]
#[ignore]
fn traversal_matches_reference_on_real_files() {
    let files = std::env::var("BPCSR_TEST_FILES").expect("BPCSR_TEST_FILES not set");
    let ctx = ctx(
        vec![Interval {
            lower: 100 * WEIGHT_FACTOR,
            upper: 3000 * WEIGHT_FACTOR,
        }],
        3,
        3,
    );

    for file in files
        .split(':')
        .filter(|f| !f.is_empty())
        .map(PathBuf::from)
    {
        let bytes = std::fs::read(&file).unwrap();
        let test_graphs = parse_bpcsr(&bytes);
        let mut cur = Cursor::new(&bytes[..]);
        let mut table = AccessionTable::default();
        let mut found = 0;
        for tg in &test_graphs {
            let mut n_acc = [0u8; 4];
            std::io::Read::read_exact(&mut cur, &mut n_acc).unwrap();
            let graph = read_single_graph(u32::from_be_bytes(n_acc), &mut cur, &mut table).unwrap();

            let expected = reference_peptides(tg, &ctx.intervals, 3, 3);
            assert_eq!(
                arena_peptides(&graph, &ctx),
                expected,
                "{} / {}",
                file.display(),
                tg.accessions[0]
            );
            found += expected.len();
        }
        println!(
            "{}: {} graphs, {} peptides",
            file.display(),
            test_graphs.len(),
            found
        );
    }
}
