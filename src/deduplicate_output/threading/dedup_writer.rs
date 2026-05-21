use std::{
    fs,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::Result;
use crossbeam_channel::Receiver;
use flate2::{Compression, write::GzEncoder};

use crate::deduplicate_output::io::{WriterWrapper, write_meta, write_sequences};
use crate::shared::BinEntryMeta;

const OUT_FASTA_FILE: &str = "peptides.fasta";
const OUT_METADATA_FILE: &str = "metadata.csv";

const META_HEADER_LINE: &str = "ID,ACC,SPOS,EPOS,MSSCLVG,QUALIFIERS";

fn get_filename(base: &str, zip: bool) -> String {
    if zip {
        format!("{}.gz", base)
    } else {
        base.to_string()
    }
}

pub fn spawn_writers(
    rx_out: Receiver<(String, Vec<BinEntryMeta>)>,
    outdir: &Path,
    zip: bool,
) -> std::thread::JoinHandle<Result<()>> {
    std::thread::spawn({
        let outdir = outdir.to_path_buf();

        move || -> Result<()> {
            fs::create_dir_all(&outdir)?;

            let seq_filename = get_filename(OUT_FASTA_FILE, zip);
            let meta_filename = get_filename(OUT_METADATA_FILE, zip);

            let seq_file = File::create(outdir.join(&seq_filename))?;
            let meta_file = File::create(outdir.join(&meta_filename))?;

            let seq_encoder = if zip {
                WriterWrapper::Compressed(GzEncoder::new(seq_file, Compression::default()))
            } else {
                WriterWrapper::Uncompressed(seq_file)
            };

            let meta_encoder = if zip {
                WriterWrapper::Compressed(GzEncoder::new(meta_file, Compression::default()))
            } else {
                WriterWrapper::Uncompressed(meta_file)
            };

            let mut seq_writer = BufWriter::new(seq_encoder);
            let mut meta_writer = BufWriter::new(meta_encoder);

            writeln!(meta_writer, "{}", META_HEADER_LINE)?;

            for (id, (sequence, metas)) in rx_out.iter().enumerate() {
                write_sequences(&mut seq_writer, id, &sequence)?;
                write_meta(&mut meta_writer, id, &metas)?;
            }

            seq_writer.flush()?;
            meta_writer.flush()?;

            Ok(())
        }
    })
}
