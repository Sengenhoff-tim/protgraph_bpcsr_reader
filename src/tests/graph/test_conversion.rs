//! .bpcsr reading, `.pgarena` (de)serialization and the conversion stage.

use std::{fs, io::Cursor};

use rand::{SeedableRng, rngs::StdRng};
use tempfile::tempdir;

use super::fixture::{random_graph, write_bpcsr};
use crate::{
    convert_graphs::{bpcsr_reader::read_single_graph, convert_graphs, prepare_graphs},
    process_graphs::{
        graph::ProteinGraph,
        io::arena_io::{MAGIC, is_arena_file, read_graph, read_header, write_graph, write_header},
    },
};

/// Reads every graph of an in-memory .bpcsr with the production reader.
fn read_all(bytes: &[u8]) -> anyhow::Result<Vec<ProteinGraph>> {
    let mut cur = Cursor::new(bytes);
    let mut out = Vec::new();
    while (cur.position() as usize) < bytes.len() {
        let mut n_acc = [0u8; 4];
        std::io::Read::read_exact(&mut cur, &mut n_acc)?;
        out.push(read_single_graph(u32::from_be_bytes(n_acc), &mut cur)?);
    }
    Ok(out)
}

fn same(a: &ProteinGraph, b: &ProteinGraph) -> bool {
    a.accession() == b.accession() && a.terminal() == b.terminal() && a.arena() == b.arena()
}

#[test]
fn reader_keeps_first_accession_and_skips_start_end_sequences() {
    let mut rng = StdRng::seed_from_u64(1);
    let tg = random_graph(&mut rng, 12, 3);
    let g = &read_all(&write_bpcsr(std::slice::from_ref(&tg))).unwrap()[0];

    assert_eq!(g.accession(), tg.accessions[0]);
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
        let got: Vec<_> = target.pdbs().collect();
        assert_eq!(got.iter().map(|p| p.0).collect::<Vec<_>>(), expected.iter().map(|p| p.0).collect::<Vec<_>>());
    }
}

#[test]
fn reader_rejects_bad_input() {
    let mut rng = StdRng::seed_from_u64(2);
    let base = random_graph(&mut rng, 6, 2);

    let mut bad_residue = base.clone();
    bad_residue.seqs[2] = "AC#D".into();

    let mut too_many_features = base.clone();
    too_many_features.vc[0] = 8;

    let mut bad_target = base.clone();
    bad_target.targets[0] = 99;

    let mut bad_offsets = base.clone();
    bad_offsets.edge_ends[1] = bad_offsets.targets.len() as u32 + 1;

    let mut bad_pdb = base.clone();
    bad_pdb.pdbs[1][0] = (10, 5);

    for g in [bad_residue, too_many_features, bad_target, bad_offsets, bad_pdb] {
        assert!(read_all(&write_bpcsr(&[g])).is_err());
    }

    // truncated file
    let bytes = write_bpcsr(&[base]);
    assert!(read_all(&bytes[..bytes.len() - 3]).is_err());
}

#[test]
fn arena_io_roundtrip() {
    let mut rng = StdRng::seed_from_u64(3);
    let graphs = read_all(&write_bpcsr(&[
        random_graph(&mut rng, 30, 4),
        random_graph(&mut rng, 2, 1),
        random_graph(&mut rng, 50, 2),
    ]))
    .unwrap();

    let mut buf = Vec::new();
    let mut scratch = Vec::new();
    write_header(&mut buf).unwrap();
    for g in &graphs {
        write_graph(&mut buf, g, &mut scratch).unwrap();
    }

    let mut cur = Cursor::new(&buf[..]);
    read_header(&mut cur).unwrap();
    for g in &graphs {
        let back = read_graph(&mut cur).unwrap().unwrap();
        assert!(same(g, &back));
    }
    assert!(read_graph(&mut cur).unwrap().is_none());

    // bad magic and truncation
    assert!(read_header(&mut Cursor::new(b"PGARENA\x02")).is_err());
    let mut cur = Cursor::new(&buf[..buf.len() - 1]);
    read_header(&mut cur).unwrap();
    let results: Vec<_> = std::iter::from_fn(|| match read_graph(&mut cur) {
        Ok(Some(_)) => Some(true),
        Ok(None) => None,
        Err(_) => Some(false),
    })
    .take(graphs.len())
    .collect();
    assert_eq!(results.last(), Some(&false));
}

#[test]
fn convert_stage_end_to_end() {
    let dir = tempdir().unwrap();
    let mut rng = StdRng::seed_from_u64(4);
    let bytes = write_bpcsr(&(0..5).map(|i| random_graph(&mut rng, 5 + 10 * i, 3)).collect::<Vec<_>>());
    let input = dir.path().join("database.bpcsr");
    fs::write(&input, &bytes).unwrap();

    // converting a .bpcsr writes the arena into the outdir
    let outdir = dir.path().join("out");
    let converted = prepare_graphs(&input, &outdir).unwrap();
    assert_eq!(converted.parent(), Some(outdir.as_path()));
    assert!(is_arena_file(&converted).unwrap());
    assert!(!is_arena_file(&input).unwrap());

    let expected = read_all(&bytes).unwrap();
    let mut cur = Cursor::new(fs::read(&converted).unwrap());
    read_header(&mut cur).unwrap();
    for g in &expected {
        assert!(same(g, &read_graph(&mut cur).unwrap().unwrap()));
    }
    assert!(read_graph(&mut cur).unwrap().is_none());

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

    // empty .bpcsr converts to zero graphs
    let empty_out = dir.path().join("empty.pgarena");
    fs::write(&empty, b"").unwrap();
    assert_eq!(convert_graphs(&empty, &empty_out).unwrap(), 0);
}
