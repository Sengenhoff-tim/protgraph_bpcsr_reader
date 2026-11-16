//! Correctness of the bucketed in-memory dedup.

use std::collections::HashMap;

use rand::{RngExt, SeedableRng, rngs::StdRng};
use xxhash_rust::xxh64::xxh64;

use crate::shared::{DedupTable, Entry, TableConfig, decode_accs};

const W: usize = 2;

fn cfg(bucket_bits: u32, bucket_size: usize, heap_budget: usize) -> TableConfig {
    TableConfig {
        bucket_bits,
        bucket_size,
        heap_budget,
        acc_width: W,
    }
}

/// Result of feeding records through a table and draining it.
#[derive(Default)]
struct Outcome {
    /// evicted during inserts
    evicted: Vec<Entry>,
    /// drained from buckets that evicted
    drained_spill: Vec<Entry>,
    /// drained from buckets that never evicted: final results
    direct: Vec<Entry>,
}

fn feed(table: &mut DedupTable, records: &[(u64, Vec<u8>, u32)]) -> Outcome {
    let mut out = Outcome::default();
    let budget = table.config().heap_budget;
    for (hash, seq, acc) in records {
        table
            .insert(*hash, seq, *acc, &mut |_, e| {
                out.evicted.push(e);
                Ok(())
            })
            .unwrap();
        assert!(table.heap_bytes() <= budget, "heap budget exceeded");
    }
    table
        .drain(|_, e, evicted| {
            if evicted {
                out.drained_spill.push(e);
            } else {
                out.direct.push(e);
            }
            Ok(())
        })
        .unwrap();
    out
}

fn random_records(
    rng: &mut StdRng,
    n: usize,
    distinct: usize,
    colliding_hashes: bool,
) -> Vec<(u64, Vec<u8>, u32)> {
    // lengths up to 60 so some sequences exceed the inline storage
    let pool: Vec<Vec<u8>> = (0..distinct)
        .map(|i| {
            let len = rng.random_range(1..60);
            let mut s: Vec<u8> = (0..len).map(|_| rng.random_range(1..=26)).collect();
            s.extend_from_slice(&(i as u32).to_le_bytes()); // distinct
            s
        })
        .collect();
    (0..n)
        .map(|_| {
            let seq = pool[rng.random_range(0..distinct)].clone();
            // colliding hashes force the sequence comparison to do the work
            let hash = if colliding_hashes {
                (seq.len() as u64) << 58
            } else {
                xxh64(&seq, 1)
            };
            (hash, seq, rng.random_range(0..400))
        })
        .collect()
}

fn accs_by_seq<'a>(entries: impl Iterator<Item = &'a Entry>) -> HashMap<Vec<u8>, Vec<u32>> {
    let mut map: HashMap<Vec<u8>, Vec<u32>> = HashMap::new();
    for e in entries {
        assert!(!e.accs.is_empty() && e.accs.len() % W == 0);
        let mut own: Vec<u32> = decode_accs(&e.accs, W).collect();
        own.sort();
        let n = own.len();
        own.dedup();
        assert_eq!(own.len(), n, "accession repeated within an entry");
        map.entry(e.seq.to_vec()).or_default().extend(own);
    }
    // copies of a sequence in different entries may share accessions; the shard dedup merges them
    map.values_mut().for_each(|v| {
        v.sort();
        v.dedup();
    });
    map
}

/// Every mapping ends up somewhere, and a sequence written directly exists nowhere else.
fn check(records: &[(u64, Vec<u8>, u32)], out: &Outcome) {
    let mut expected: HashMap<Vec<u8>, Vec<u32>> = HashMap::new();
    for (_, seq, acc) in records {
        expected.entry(seq.clone()).or_default().push(*acc);
    }
    expected.values_mut().for_each(|v| {
        v.sort();
        v.dedup();
    });

    let all = accs_by_seq(
        out.evicted
            .iter()
            .chain(&out.drained_spill)
            .chain(&out.direct),
    );
    assert_eq!(all, expected, "mappings lost or invented");

    let elsewhere = accs_by_seq(out.evicted.iter().chain(&out.drained_spill));
    let mut direct_seen = std::collections::HashSet::new();
    for e in &out.direct {
        assert!(
            direct_seen.insert(e.seq.to_vec()),
            "sequence written directly twice"
        );
        assert!(
            !elsewhere.contains_key(e.seq.as_slice()),
            "direct sequence also spilled"
        );
    }
}

#[test]
fn dedup_is_complete_without_pressure() {
    let mut rng = StdRng::seed_from_u64(7);
    let records = random_records(&mut rng, 3000, 200, false);
    let mut table = DedupTable::new(cfg(10, 8, usize::MAX));
    let out = feed(&mut table, &records);
    check(&records, &out);
    assert!(out.evicted.is_empty() && out.drained_spill.is_empty());
    assert_eq!(out.direct.len(), 200);
    assert_eq!(table.stats().inserted, 200);
    assert_eq!(table.stats().hits, 2800);
}

#[test]
fn dedup_stays_correct_under_eviction() {
    let mut rng = StdRng::seed_from_u64(8);
    for round in 0..40 {
        let colliding = round % 3 == 0;
        let distinct = rng.random_range(5..300);
        let records = random_records(&mut rng, 2000, distinct, colliding);
        let config = cfg(
            rng.random_range(0..4),
            rng.random_range(1..6),
            rng.random_range(0..400),
        );
        let mut table = DedupTable::new(config);
        // two intervals through the same table: drain must reset everything
        for _interval in 0..2 {
            let out = feed(&mut table, &records);
            check(&records, &out);
            assert_eq!(table.heap_bytes(), 0);
        }
        assert!(table.stats().evicted > 0, "round {round} did not evict");
    }
}

#[test]
fn repeated_mappings_are_stored_once() {
    let mut table = DedupTable::new(cfg(0, 4, usize::MAX));
    let mut none = |_, _| -> anyhow::Result<()> { panic!("nothing should be evicted") };
    for acc in [5, 5, 7, 5, 7, 9, 9] {
        table.insert(1, b"PEP", acc, &mut none).unwrap();
    }
    let mut drained = Vec::new();
    table
        .drain(|_, e, _| {
            drained.push(decode_accs(&e.accs, W).collect::<Vec<_>>());
            Ok(())
        })
        .unwrap();
    assert_eq!(drained, [vec![5, 7, 9]]);
}

#[test]
fn bucket_evicts_least_recently_used() {
    let mut table = DedupTable::new(cfg(0, 2, usize::MAX));
    let mut evicted = Vec::new();
    let mut put = |t: &mut DedupTable, seq: &[u8], acc| {
        t.insert(1, seq, acc, &mut |_, e| {
            evicted.push(e.seq.to_vec());
            Ok(())
        })
        .unwrap()
    };
    put(&mut table, b"A", 0);
    put(&mut table, b"B", 1);
    put(&mut table, b"A", 2); // hit moves A to the front
    put(&mut table, b"C", 3); // evicts B, the least recently used
    drop(put);
    assert_eq!(evicted, [b"B".to_vec()]);
}

#[test]
fn heap_budget_evicts_spilled_entries() {
    let mut table = DedupTable::new(cfg(4, 4, 0));
    let mut evicted = Vec::new();
    let long = vec![3u8; 100]; // does not fit inline
    table
        .insert(5, &long, 1, &mut |_, e| {
            evicted.push(e);
            Ok(())
        })
        .unwrap();
    table
        .insert(6, b"short", 1, &mut |_, e| {
            evicted.push(e);
            Ok(())
        })
        .unwrap();
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].seq.as_slice(), &long[..]);
    assert_eq!(table.heap_bytes(), 0);
}

#[test]
fn config_from_budget() {
    let c = TableConfig::from_budget(100 << 20, 8, 3);
    assert!(c.n_buckets().is_power_of_two());
    assert!(c.fixed_bytes() <= (100 << 20) / 100 * 80);
    assert!(
        c.fixed_bytes() * 2 > (100 << 20) / 100 * 80,
        "should use at least half of its share"
    );
    assert_eq!(c.heap_budget, (100 << 20) - (100 << 20) / 100 * 80);
    assert_eq!(TableConfig::SLOT_BYTES, 64, "slot layout changed");

    let tiny = TableConfig::from_budget(10, 8, 1);
    assert_eq!(tiny.n_buckets(), 1);
}
