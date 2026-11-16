//! The thread writing `peptides.fasta` and `metadata.bin` (see `output_files`).

use std::{
    fs::{File, create_dir_all},
    path::Path,
    thread::{self, JoinHandle},
};

use anyhow::{Context, Result, anyhow, ensure};
use crossbeam_channel::{Receiver, Sender, bounded};

use super::{
    METADATA_FILE, PEPTIDES_FILE, compressed_stream::CompressedStream, io::write_sequences,
    output_path,
};
use crate::shared::OutputBatch;

const QUEUE_DEPTH: usize = 64;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OutputStats {
    pub peptides: u64,
    pub accessions: u64,
}

/// Owns the output files; accepts deduplicated batches from any thread and assigns ids.
pub struct OutputSink {
    tx: Sender<OutputBatch>,
    handle: JoinHandle<Result<OutputStats>>,
}

impl OutputSink {
    pub fn spawn(outdir: &Path, zip: bool, acc_width: usize) -> Result<Self> {
        ensure!(
            (1..=4).contains(&acc_width),
            "invalid accession width {acc_width}"
        );
        create_dir_all(outdir)?;
        let create = |name| {
            let path = output_path(outdir, name, zip);
            File::create(&path).with_context(|| format!("creating {}", path.display()))
        };
        let peptides = CompressedStream::open(create(PEPTIDES_FILE)?, zip)?;
        let metadata = CompressedStream::open(create(METADATA_FILE)?, zip)?;

        let (tx, rx) = bounded::<OutputBatch>(QUEUE_DEPTH);
        let handle = thread::Builder::new()
            .name("output".into())
            .spawn(move || sink_loop(rx, peptides, metadata, acc_width))?;
        Ok(Self { tx, handle })
    }

    pub fn sender(&self) -> Sender<OutputBatch> {
        self.tx.clone()
    }

    /// Waits until every sender is dropped and all batches are written.
    pub fn finish(self) -> Result<OutputStats> {
        drop(self.tx);
        self.handle
            .join()
            .map_err(|_| anyhow!("output thread panicked"))?
    }
}

fn sink_loop(
    rx: Receiver<OutputBatch>,
    mut peptides: CompressedStream,
    mut metadata: CompressedStream,
    w: usize,
) -> Result<OutputStats> {
    metadata.buf().push(w as u8);

    let mut stats = OutputStats::default();
    let mut id: u128 = 0;
    let mut scratch = Vec::new();

    for batch in rx {
        for (seq, accs) in batch.iter() {
            let n = (accs.len() / w) as u128;
            ensure!(
                n > 0 && accs.len().is_multiple_of(w),
                "peptide without accessions"
            );
            write_sequences(peptides.buf(), id, n - 1, seq, &mut scratch)?;
            metadata.buf().extend_from_slice(accs);
            id += n;
            stats.peptides += 1;
            peptides.rotate_if_full()?;
            metadata.rotate_if_full()?;
        }
    }
    stats.accessions = id as u64;

    peptides.finish()?;
    metadata.finish()?;
    Ok(stats)
}
