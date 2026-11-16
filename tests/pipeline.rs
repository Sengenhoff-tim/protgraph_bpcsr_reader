//! End to end: .bpcsr → conversion → per-interval traversal and dedup → output files,
//! compared against the naive reference traversal.

use std::{collections::HashMap, fs, io::Read, path::Path};

use flate2::read::MultiGzDecoder;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use tempfile::tempdir;

mod common;

use bpcsr_to_fasta::{
    convert_graphs::prepare_graphs,
    deduplicate_output::{ACCESSIONS_FILE, METADATA_FILE, PEPTIDES_FILE, output_path},
    pipeline::{RunParams, RunReport, run},
    process_graphs::{
        ProcessParams,
        utilities::{Interval, IntervalVecExt},
    },
    shared::decode_accs,
};
use common::fixture::{TestGraph, random_graph, reference_peptides, write_bpcsr};

/// Variant limit of all runs (the CLI default).
const MAX_VARS: u16 = 3;

/// (sequence, sorted accession names), sorted
type Peptides = Vec<(String, Vec<String>)>;

fn params(
    outdir: &Path,
    zip: bool,
    threads: usize,
    table_budget: usize,
    bucket_size: usize,
) -> RunParams {
    RunParams {
        outdir: outdir.to_path_buf(),
        zip,
        process: ProcessParams {
            threads,
            max_vars: MAX_VARS,
            max_cleavages: 2,
            bucket_size,
            table_budget,
            batch_budget: 0,
            spill_budget: 0,
            max_handles: Some(8),
            jobs_per_graph: 3,
        },
        dedup_budget: 64 << 20,
    }
}

fn read_file(path: &Path, zip: bool) -> Vec<u8> {
    let raw = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    if !zip {
        return raw;
    }
    let mut out = Vec::new();
    MultiGzDecoder::new(&raw[..]).read_to_end(&mut out).unwrap();
    out
}

/// Parses the three output files and checks their internal consistency.
fn read_output(outdir: &Path, zip: bool) -> Peptides {
    let fasta =
        String::from_utf8(read_file(&output_path(outdir, PEPTIDES_FILE, zip), zip)).unwrap();
    let meta = read_file(&output_path(outdir, METADATA_FILE, zip), zip);
    let accessions: Vec<String> =
        String::from_utf8(read_file(&output_path(outdir, ACCESSIONS_FILE, zip), zip))
            .unwrap()
            .lines()
            .skip(1)
            .enumerate()
            .map(|(i, line)| {
                let (id, acc) = line.split_once(',').unwrap();
                assert_eq!(id.parse::<usize>().unwrap(), i);
                acc.to_string()
            })
            .collect();

    let w = meta[0] as usize;
    let ids: Vec<u32> = decode_accs(&meta[1..], w).collect();
    assert_eq!((meta.len() - 1) % w, 0);

    let mut next_start = 0usize;
    let mut out = Vec::new();
    for record in fasta.split('>').skip(1) {
        let mut lines = record.lines();
        let header = lines.next().unwrap();
        let (start, count) = header.strip_prefix("pg|").unwrap().split_once(':').unwrap();
        let (start, count) = (
            start.parse::<usize>().unwrap(),
            count.parse::<usize>().unwrap() + 1,
        );
        assert_eq!(start, next_start, "id ranges must be contiguous");
        next_start += count;
        let seq: String = lines.collect();
        let mut accs: Vec<String> = ids[start..start + count]
            .iter()
            .map(|&id| accessions[id as usize].clone())
            .collect();
        accs.sort();
        out.push((seq, accs));
    }
    assert_eq!(
        next_start,
        ids.len(),
        "metadata holds ids no peptide refers to"
    );
    out.sort();
    out
}

/// Reference: per interval, every matching path maps its sequence to its graph's
/// accession; sequences are unique within an interval, mappings are unique.
fn expected(graphs: &[TestGraph], intervals: &[Interval]) -> Peptides {
    let mut out = Vec::new();
    for interval in intervals {
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        for g in graphs {
            for seq in reference_peptides(g, std::slice::from_ref(interval), MAX_VARS, 2) {
                groups.entry(seq).or_default().push(g.accessions[0].clone());
            }
        }
        out.extend(groups.into_iter().map(|(seq, mut accs)| {
            accs.sort();
            accs.dedup();
            (seq, accs)
        }));
    }
    out.sort();
    out
}

struct Case {
    graphs: Vec<TestGraph>,
    intervals: Vec<Interval>,
}

fn random_case(seed: u64) -> Case {
    let mut rng = StdRng::seed_from_u64(seed);
    let graphs = (0..rng.random_range(3..10))
        .map(|_| {
            let (n, p) = (rng.random_range(5..40), rng.random_range(2..5));
            let mut g = random_graph(&mut rng, n, p);
            // shared accessions across graphs
            g.accessions[0] = format!("P{}", rng.random_range(0..4));
            g
        })
        .collect();
    let intervals = (0..rng.random_range(1..5))
        .map(|_| {
            let lower = rng.random_range(0..700);
            Interval {
                lower,
                upper: lower + rng.random_range(50..300),
            }
        })
        .collect::<Vec<_>>()
        .merge_and_sort()
        .unwrap()
        .into_vec();
    Case { graphs, intervals }
}

fn run_case(
    case: &Case,
    zip: bool,
    threads: usize,
    table_budget: usize,
    bucket_size: usize,
) -> (Peptides, RunReport) {
    let dir = tempdir().unwrap();
    let input = dir.path().join("database.bpcsr");
    fs::write(&input, write_bpcsr(&case.graphs)).unwrap();
    let outdir = dir.path().join("out");
    let graph_path = prepare_graphs(&input, &outdir).unwrap();

    let report = run(
        &params(&outdir, zip, threads, table_budget, bucket_size),
        &case.intervals,
        &graph_path,
    )
    .unwrap();
    assert!(!outdir.join("tmp").exists(), "tmp dir left behind");
    (read_output(&outdir, zip), report)
}

#[test]
fn in_memory_dedup_matches_reference() {
    let mut total = 0;
    for seed in 0..15 {
        let case = random_case(seed);
        let (got, report) = run_case(&case, false, 2, 64 << 20, 8);
        assert_eq!(got, expected(&case.graphs, &case.intervals), "seed {seed}");
        assert_eq!(report.spilled_intervals, 0, "large tables must not spill");
        assert_eq!(report.output.peptides as usize, got.len());
        total += got.len();
    }
    println!("{total} peptides");
    // guards against vacuous runs; the exact count depends on the random cases
    assert!(total > 200, "test cases produce too few peptides ({total})");
}

#[test]
fn spilling_dedup_matches_reference() {
    let (mut spilled, mut intervals) = (0, 0);
    for seed in 100..115 {
        let case = random_case(seed);
        // no budget: one bucket of 1-2 entries per writer, no heap
        let threads = if seed % 2 == 0 { 1 } else { 3 };
        let (got, report) = run_case(&case, seed % 2 == 0, threads, 0, 1 + seed as usize % 2);
        assert_eq!(got, expected(&case.graphs, &case.intervals), "seed {seed}");
        spilled += report.spilled_intervals;
        intervals += report.intervals;
    }
    println!("{spilled} of {intervals} intervals spilled");
    // intervals with at most one peptide per writer cannot evict
    assert!(
        spilled >= 5,
        "tiny tables should spill ({spilled} of {intervals} intervals spilled)"
    );
}

#[test]
fn intervals_without_matches_produce_empty_output() {
    let case = Case {
        intervals: vec![Interval {
            lower: -100,
            upper: -50,
        }],
        ..random_case(3)
    };
    let (got, report) = run_case(&case, true, 1, 1 << 20, 4);
    assert!(got.is_empty());
    assert_eq!(report.output.peptides, 0);
}

#[test]
fn max_vars_above_the_edge_format_limit_is_rejected() {
    let case = random_case(1);
    let dir = tempdir().unwrap();
    let input = dir.path().join("database.bpcsr");
    fs::write(&input, write_bpcsr(&case.graphs)).unwrap();
    let outdir = dir.path().join("out");
    let graph_path = prepare_graphs(&input, &outdir).unwrap();

    let mut p = params(&outdir, false, 1, 1 << 20, 4);
    p.process.max_vars = 7;
    let err = run(&p, &case.intervals, &graph_path)
        .unwrap_err()
        .to_string();
    assert!(err.contains("max_vars"), "{err}");
}
