use std::{
    fs::{self, File}, io::{BufWriter, Write}, path::Path, thread
};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use flate2::{Compression, write::GzEncoder};

use crate::deduplicate_output::{io::{WriterWrapper, write_meta, write_sequences}, threading::dedup_workers::WorkerResult};
use crate::shared::EntryBuffer;

const OUT_FASTA_FILE: &str = "peptides.fasta";
const OUT_METADATA_FILE: &str = "metadata.csv";

const META_HEADER_LINE: &str = "ID,ACC,STARTPOS,ENDPOS,MISSCLEAVAGE,QUALIFIERS";

fn get_filename(base: &str, zip: bool) -> String {
    if zip {
        format!("{}.gz", base)
    } else {
        base.to_string()
    }
}

pub fn spawn_writers(
    rx_entry: Receiver<WorkerResult>,
    tx_buffer_empty: Sender<EntryBuffer>,
    outdir: &Path,
    zip: bool,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn({
        let outdir = outdir.to_path_buf();

        move || -> Result<()> {
            fs::create_dir_all(&outdir)?;
            
            let seq_filename = get_filename(OUT_FASTA_FILE, zip);    

            let seq_file = File::create(outdir.join(&seq_filename))?;     

            let seq_encoder = if zip {
                WriterWrapper::Compressed(GzEncoder::new(seq_file, Compression::default()))
            } else {
                WriterWrapper::Uncompressed(seq_file)
            };
            
            let mut seq_writer = BufWriter::new(seq_encoder);
            
            let meta_filename = get_filename(OUT_METADATA_FILE, zip);

            let meta_file = File::create(outdir.join(&meta_filename))?;

            let meta_encoder = if zip {
                WriterWrapper::Compressed(GzEncoder::new(meta_file, Compression::default()))
            } else {
                WriterWrapper::Uncompressed(meta_file)
            };

            let mut meta_writer = BufWriter::new(meta_encoder);

            writeln!(meta_writer, "{}", META_HEADER_LINE)?;

            let mut id: u128 = 1;

            for mut result in rx_entry {
                let buffer = &mut result.buffer;

                for (seq, metas) in result.groups {
                    id += 1;
                    write_sequences(&mut seq_writer, id, &seq.get(buffer))?;
                    for meta in metas {
                        write_meta(&mut meta_writer, id, &meta.get(buffer))?;
                    }
                }
                buffer.clear();

                //TODO add graceful exit
                if tx_buffer_empty.send(result.buffer).is_err() {
                    seq_writer.flush()?;
                    meta_writer.flush()?;
                    return Ok(());
                };
            }

            seq_writer.flush()?;
            meta_writer.flush()?;

            Ok(())
        }
    })
}
