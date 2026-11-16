//! Record formats: accession ids, worker batches, shard records, output batches.

use crate::shared::{
    DiskBuffer, DiskRecord, MemBuffer, MemRecord, OutputBatch, acc_width, decode_accs, dedup_accs,
    encode_acc,
};

#[test]
fn accession_width() {
    for (n, w) in [
        (0, 1),
        (1, 1),
        (256, 1),
        (257, 2),
        (1 << 16, 2),
        ((1 << 16) + 1, 3),
        (1 << 24, 3),
        ((1 << 24) + 1, 4),
    ] {
        assert_eq!(acc_width(n), w, "n = {n}");
    }
}

#[test]
fn accession_ids_roundtrip() {
    for w in 1..=4usize {
        let max = if w == 4 {
            u32::MAX
        } else {
            (1u32 << (8 * w)) - 1
        };
        let ids = [0, 1, max / 2, max];
        let bytes: Vec<u8> = ids
            .iter()
            .flat_map(|&id| encode_acc(id)[..w].to_vec())
            .collect();
        assert_eq!(bytes.len(), ids.len() * w);
        assert_eq!(decode_accs(&bytes, w).collect::<Vec<_>>(), ids);
    }
}

#[test]
fn accession_lists_are_deduplicated() {
    for w in 1..=4usize {
        let ids = [200u32, 2, 200, 1, 2, 0];
        let mut bytes: Vec<u8> = ids
            .iter()
            .flat_map(|&id| encode_acc(id)[..w].to_vec())
            .collect();
        dedup_accs(&mut bytes, w);
        let mut expected = ids.to_vec();
        expected.sort();
        expected.dedup();
        assert_eq!(
            decode_accs(&bytes, w).collect::<Vec<_>>(),
            expected,
            "w = {w}"
        );
    }
}

#[test]
fn mem_buffer_roundtrip() {
    let mut buf = MemBuffer::with_capacity(1024);
    buf.push(1, 10, b"abc");
    buf.push(2, 20, b"");
    buf.push(3, 30, b"defgh");
    let recs: Vec<_> = buf.iter().collect();
    assert_eq!(
        recs,
        [
            MemRecord {
                hash: 1,
                acc: 10,
                seq: b"abc"
            },
            MemRecord {
                hash: 2,
                acc: 20,
                seq: b""
            },
            MemRecord {
                hash: 3,
                acc: 30,
                seq: b"defgh"
            },
        ]
    );
    assert_eq!(buf.len(), 3);
    assert!(buf.byte_len() >= 8);
    buf.clear();
    assert!(buf.is_empty());
}

#[test]
fn disk_buffer_roundtrip() {
    let w = 3;
    let mut buf = DiskBuffer::default();
    buf.push(b"seq1", &[1, 0, 0, 2, 0, 0], w).unwrap();
    buf.push(&[7; 300], &[9, 9, 9], w).unwrap();
    assert!(
        buf.push(b"x", &[1, 2], w).is_err(),
        "partial id must be rejected"
    );

    let recs: Vec<_> = buf.iter(w).collect::<anyhow::Result<_>>().unwrap();
    assert_eq!(
        recs[0],
        DiskRecord {
            seq: b"seq1",
            accs: &[1, 0, 0, 2, 0, 0]
        }
    );
    assert_eq!(recs[1].seq, &[7; 300][..]);
    assert_eq!(recs.len(), 2);

    // a truncated buffer reports an error instead of silently stopping
    let bytes = buf.as_bytes()[..buf.len() - 1].to_vec();
    let mut truncated = DiskBuffer::default();
    truncated.read_from(&mut &bytes[..]).unwrap();
    let results: Vec<_> = truncated.iter(w).collect();
    assert_eq!(results.len(), 2);
    assert!(results[0].is_ok() && results[1].is_err());
}

#[test]
fn output_batch_roundtrip() {
    let mut out = OutputBatch::default();
    out.push(b"ab", &[1]);
    out.push(b"", &[2, 3]);
    out.push(b"cde", &[]);
    let items: Vec<_> = out.iter().collect();
    assert_eq!(
        items,
        [
            (&b"ab"[..], &[1u8][..]),
            (&b""[..], &[2, 3][..]),
            (&b"cde"[..], &[][..])
        ]
    );
    assert_eq!(out.len(), 3);
}
