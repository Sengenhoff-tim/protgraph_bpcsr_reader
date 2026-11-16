//! .bpcsr reading, accession interning, `.pgarena` (de)serialization and the conversion stage.

use std::{fs, io::Cursor};

use rand::{SeedableRng, rngs::StdRng};
use tempfile::tempdir;

use super::fixture::{random_graph, write_bpcsr};
use crate::{
    convert_graphs::{
        convert_graphs, prepare_graphs,
        {AccessionTable, bpcsr_reader::read_single_graph},
    },
    process_graphs::{
        graph::ProteinGraph,
        io::arena_io::{ArenaFile, ArenaWriter, MAGIC, is_arena_file, read_graph, write_graph},
    },
};

/// Reads every graph of an in-memory .bpcsr with the production reader.
fn read_all(bytes: &[u8]) -> anyhow::Result<(Vec<ProteinGraph>, AccessionTable)> {
    let mut cur = Cursor::new(bytes);
    let mut table = AccessionTable::default();
    let mut out = Vec::new();
    while (cur.position() as usize) < bytes.len() {
        let mut n_acc = [0u8; 4];
        std::io::Read::read_exact(&mut cur, &mut n_acc)?;
        out.push(read_single_graph(
            u32::from_be_bytes(n_acc),
            &mut cur,
            &mut table,
        )?);
    }
    Ok((out, table))
}

fn same(a: &ProteinGraph, b: &ProteinGraph) -> bool {
    a.acc_id() == b.acc_id() && a.terminal() == b.terminal() && a.arena() == b.arena()
}

#[test]
fn reader_keeps_first_accession_and_skips_start_end_sequences() {
    let mut rng = StdRng::seed_from_u64(1);
    let tg = random_graph(&mut rng, 12, 3);
    let (graphs, table) = read_all(&write_bpcsr(std::slice::from_ref(&tg))).unwrap();
    let g = &graphs[0];

    assert_eq!(table.names(), &tg.accessions[..1]);
    assert_eq!(g.acc_id(), 0);
    assert_eq!(g.node(0).seq().count(), 0);
    assert_eq!(g.node(g.terminal()).seq().count(), 0);

    // every edge of the root keeps its flags, sentinel pdbs are dropped and the rest sorted
    let root = g.node(0);
    for (e, i) in root.edges().zip(tg.edges_of(0)) {
        assert_eq!(e.features(), tg.vc[i] as u16);
        assert_eq!(e.cleaves(), (tg.cleaved[i] != 0) as u16);
        let target = g.node(e.target());
        assert_eq!(target.mono_weight(), tg.weights[tg.targets[i] as usize]);

        let mut expected: Vec<_> = tg.pdbs[tg.targets[i] as usize]
            .iter()
            .copied()
            .filter(|&(lo, _)| lo != i64::MAX)
            .collect();
        expected.sort_by_key(|&(lo, _)| lo);
        let got: Vec<_> = target.pdbs().map(|p| p.0).collect();
        assert_eq!(got, expected.iter().map(|p| p.0).collect::<Vec<_>>());
    }
}

#[test]
fn accessions_are_interned_in_order_of_appearance() {
    let mut rng = StdRng::seed_from_u64(5);
    let mut graphs: Vec<_> = (0..4).map(|_| random_graph(&mut rng, 5, 1)).collect();
    for (g, acc) in graphs.iter_mut().zip(["B", "A", "B", "C"]) {
        g.accessions[0] = acc.into();
    }
    let (read, table) = read_all(&write_bpcsr(&graphs)).unwrap();
    assert_eq!(
        read.iter().map(|g| g.acc_id()).collect::<Vec<_>>(),
        [0, 1, 0, 2]
    );
    assert_eq!(table.names(), ["B", "A", "C"]);
}

#[test]
fn reader_rejects_bad_input() {
    let mut rng = StdRng::seed_from_u64(2);
    let base = random_graph(&mut rng, 6, 2);

    let mut bad_residue = base.clone();
    bad_residue.seqs[2] = "AC#D".into();

    let mut bad_target = base.clone();
    bad_target.targets[0] = 99;

    let mut bad_offsets = base.clone();
    bad_offsets.edge_ends[1] = bad_offsets.targets.len() as u32 + 1;

    let mut bad_pdb = base.clone();
    bad_pdb.pdbs[1][0] = (10, 5);

    for g in [bad_residue, bad_target, bad_offsets, bad_pdb] {
        assert!(read_all(&write_bpcsr(&[g])).is_err());
    }

    // large variant counts are capped, not rejected
    let mut many_features = base.clone();
    many_features.vc[0] = 200;
    assert!(read_all(&write_bpcsr(&[many_features])).is_ok());

    // truncated file
    let bytes = write_bpcsr(&[base]);
    assert!(read_all(&bytes[..bytes.len() - 3]).is_err());
}

#[test]
fn graph_record_roundtrip() {
    let mut rng = StdRng::seed_from_u64(3);
    let (graphs, _) = read_all(&write_bpcsr(&[
        random_graph(&mut rng, 30, 4),
        random_graph(&mut rng, 2, 1),
        random_graph(&mut rng, 50, 2),
    ]))
    .unwrap();

    let mut buf = Vec::new();
    let mut scratch = Vec::new();
    for g in &graphs {
        write_graph(&mut buf, g, &mut scratch).unwrap();
    }

    let mut cur = Cursor::new(&buf[..]);
    for g in &graphs {
        assert!(same(g, &read_graph(&mut cur).unwrap().unwrap()));
    }
    assert!(read_graph(&mut cur).unwrap().is_none());

    // truncated
    let mut cur = Cursor::new(&buf[..buf.len() - 1]);
    let results: Vec<_> = (0..graphs.len())
        .map(|_| read_graph(&mut cur).is_ok())
        .collect();
    assert_eq!(results.last(), Some(&false));
}

#[test]
fn arena_file_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("g.pgarena");
    let mut rng = StdRng::seed_from_u64(6);
    let (graphs, table) = read_all(&write_bpcsr(
        &(0..6)
            .map(|i| random_graph(&mut rng, 3 + i * 7, 2))
            .collect::<Vec<_>>(),
    ))
    .unwrap();

    let mut writer = ArenaWriter::create(&path).unwrap();
    for g in &graphs {
        writer.write_graph(g).unwrap();
    }
    writer.finish(table.names()).unwrap();

    let arena = ArenaFile::open(&path).unwrap();
    assert_eq!(arena.accessions(), table.names());
    // every pass reads all graphs again, and stops before the accession table
    for _pass in 0..2 {
        let mut reader = arena.graphs().unwrap();
        for g in &graphs {
            assert!(same(g, &reader.next_graph().unwrap().unwrap()));
        }
        assert!(reader.next_graph().unwrap().is_none());
    }

    // a file that was never finished has no table
    let unfinished = dir.path().join("unfinished.pgarena");
    let mut writer = ArenaWriter::create(&unfinished).unwrap();
    writer.write_graph(&graphs[0]).unwrap();
    drop(writer);
    assert!(ArenaFile::open(&unfinished).is_err());

    // other format versions are recognized but rejected
    let old = dir.path().join("old.pgarena");
    fs::write(&old, b"PGARENA\x01rest").unwrap();
    assert!(is_arena_file(&old).is_err());
}

#[test]
fn convert_stage_end_to_end() {
    let dir = tempdir().unwrap();
    let mut rng = StdRng::seed_from_u64(4);
    let bytes = write_bpcsr(
        &(0..5)
            .map(|i| random_graph(&mut rng, 5 + 10 * i, 3))
            .collect::<Vec<_>>(),
    );
    let input = dir.path().join("database.bpcsr");
    fs::write(&input, &bytes).unwrap();

    // converting a .bpcsr writes the arena into the outdir
    let outdir = dir.path().join("out");
    let converted = prepare_graphs(&input, &outdir).unwrap();
    assert_eq!(converted.parent(), Some(outdir.as_path()));
    assert!(is_arena_file(&converted).unwrap());
    assert!(!is_arena_file(&input).unwrap());

    let (expected, table) = read_all(&bytes).unwrap();
    let arena = ArenaFile::open(&converted).unwrap();
    assert_eq!(arena.accessions(), table.names());
    let mut reader = arena.graphs().unwrap();
    for g in &expected {
        assert!(same(g, &reader.next_graph().unwrap().unwrap()));
    }
    assert!(reader.next_graph().unwrap().is_none());

    // an arena input is used as-is, no second conversion
    let again = prepare_graphs(&converted, &dir.path().join("out2")).unwrap();
    assert_eq!(again, converted);
    assert!(!dir.path().join("out2").exists());

    // empty and tiny files are not arena files
    let empty = dir.path().join("empty");
    fs::write(&empty, b"").unwrap();
    assert!(!is_arena_file(&empty).unwrap());
    fs::write(&empty, &MAGIC[..4]).unwrap();
    assert!(!is_arena_file(&empty).unwrap());

    // empty .bpcsr converts to zero graphs and an empty table
    let empty_out = dir.path().join("empty.pgarena");
    fs::write(&empty, b"").unwrap();
    assert_eq!(convert_graphs(&empty, &empty_out).unwrap(), (0, 0));
    assert!(ArenaFile::open(&empty_out).unwrap().accessions().is_empty());
}
