use std::{
    fs::{File, create_dir_all},
    io::{BufReader, ErrorKind},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use byteorder::{BigEndian, ReadBytesExt};
use tracing::info;

use super::{AccessionTable, bpcsr_reader::read_single_graph};
use crate::process_graphs::io::arena_io::{ArenaWriter, EXTENSION, is_arena_file};

/// Returns the path of an arena file for `input`: `input` itself if it already is one,
/// otherwise the result of converting it into `outdir`.
pub fn prepare_graphs(input: &Path, outdir: &Path) -> Result<PathBuf> {
    if is_arena_file(input)? {
        info!("INPUT {} is already converted", input.display());
        return Ok(input.to_path_buf());
    }

    create_dir_all(outdir).with_context(|| format!("failed to create {}", outdir.display()))?;
    let output = outdir.join("graphs").with_extension(EXTENSION);
    let (graphs, accessions) = convert_graphs(input, &output)?;
    info!(
        "CONVERTED {} graphs ({} accessions) to {}",
        graphs,
        accessions,
        output.display()
    );
    Ok(output)
}

/// Converts every graph of a .bpcsr file into the arena format.
/// Returns the number of graphs and of distinct accessions.
pub fn convert_graphs(input: &Path, output: &Path) -> Result<(usize, usize)> {
    let mut reader =
        BufReader::new(File::open(input).with_context(|| format!("opening {}", input.display()))?);
    let mut writer = ArenaWriter::create(output)?;
    let mut accessions = AccessionTable::default();

    let mut total = 0usize;
    loop {
        let num_acc = match reader.read_u32::<BigEndian>() {
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(anyhow!("Failed to read graph count: {}", e)),
        };

        let graph = read_single_graph(num_acc, &mut reader, &mut accessions)
            .with_context(|| format!("converting graph {total}"))?;
        writer.write_graph(&graph)?;
        total += 1;
    }

    let n_accessions = accessions.names().len();
    writer.finish(accessions.names())?;
    Ok((total, n_accessions))
}
