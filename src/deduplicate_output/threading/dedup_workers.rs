use std::{collections::HashMap, fs, mem, path::PathBuf};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};

use crate::shared::{DiskBuffer, OutputBatch, dedup_accs};

const OUTPUT_BATCH_BYTES: usize = 1 << 20;

/// Deduplicates whole shard files: every copy of a sequence of an interval is in the
/// same shard, so merging the accession lists per sequence (without repeats) gives the
/// final result.
/// Processed shard files are deleted.
pub fn shard_worker(
    rx_paths: Receiver<PathBuf>,
    tx_out: Sender<OutputBatch>,
    acc_width: usize,
) -> Result<()> {
    let mut buffer = DiskBuffer::default();
    for path in rx_paths {
        let file = fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let mut decoder = zstd::stream::read::Decoder::new(file)?;
        buffer
            .read_from(&mut decoder)
            .with_context(|| format!("reading {}", path.display()))?;

        let mut groups: HashMap<&[u8], Vec<u8>> = HashMap::new();
        for record in buffer.iter(acc_width) {
            let record = record.with_context(|| format!("parsing {}", path.display()))?;
            groups
                .entry(record.seq)
                .or_default()
                .extend_from_slice(record.accs);
        }

        let mut out = OutputBatch::default();
        for (seq, mut accs) in groups {
            // an evicted entry and a later copy of the sequence can share accessions
            dedup_accs(&mut accs, acc_width);
            out.push(seq, &accs);
            if out.byte_len() >= OUTPUT_BATCH_BYTES {
                tx_out.send(mem::take(&mut out)).context("output closed")?;
            }
        }
        if !out.is_empty() {
            tx_out.send(out).context("output closed")?;
        }
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}
