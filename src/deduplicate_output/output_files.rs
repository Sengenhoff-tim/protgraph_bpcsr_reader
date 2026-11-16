//! Output files, written by one long-lived thread:
//!
//! - `peptides.fasta`: `>pg|{start}:{count - 1}` + sequence (60 residues per line)
//! - `metadata.bin`: one byte with the accession id width `w`, then the accession ids of
//!   all peptides in fasta order, `w` bytes little endian each. Peptide `k` owns the ids
//!   `start..start + count` of its header.
//! - `accessions.csv`: `id,accession`
//!
//! With `zip`, every file is gzip compressed (`.gz` appended).

use std::{
    fs::{File, create_dir_all},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use flate2::{Compression, write::GzEncoder};

use super::io::WriterWrapper;

/// gzip level of the output files with `--zip` (default would be 6).
pub const OUTPUT_GZIP_LEVEL: u32 = 3;

pub const PEPTIDES_FILE: &str = "peptides.fasta";
pub const METADATA_FILE: &str = "metadata.bin";
pub const ACCESSIONS_FILE: &str = "accessions.csv";

pub fn output_path(outdir: &Path, name: &str, zip: bool) -> PathBuf {
    if zip {
        outdir.join(format!("{name}.gz"))
    } else {
        outdir.join(name)
    }
}

/// Writes the accession table (`id,accession`).
pub fn write_accessions(outdir: &Path, accessions: &[String], zip: bool) -> Result<()> {
    create_dir_all(outdir)?;
    let path = output_path(outdir, ACCESSIONS_FILE, zip);
    let file = File::create(&path).with_context(|| format!("creating {}", path.display()))?;
    let inner = if zip {
        WriterWrapper::Compressed(GzEncoder::new(file, Compression::new(OUTPUT_GZIP_LEVEL)))
    } else {
        WriterWrapper::Uncompressed(file)
    };
    let mut writer = BufWriter::new(inner);
    writeln!(writer, "id,accession")?;
    for (id, acc) in accessions.iter().enumerate() {
        writeln!(writer, "{id},{acc}")?;
    }
    writer.flush()?;
    if let WriterWrapper::Compressed(enc) = writer.into_inner().map_err(|e| e.into_error())? {
        enc.finish()?;
    }
    Ok(())
}
