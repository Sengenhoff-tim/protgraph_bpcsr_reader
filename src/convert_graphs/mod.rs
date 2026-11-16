//! Conversion stage: reads the .bpcsr produced by protgraph once and writes the
//! graphs in arena layout (`.pgarena`), which later passes read with a bulk copy.
pub(crate) mod bpcsr_reader;

use std::{
    fs::{File, create_dir_all},
    io::{BufReader, BufWriter, ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use byteorder::{BigEndian, ReadBytesExt};
use tracing::info;

use crate::process_graphs::io::arena_io::{EXTENSION, is_arena_file, write_graph, write_header};
use bpcsr_reader::read_single_graph;

/// Returns the path of an arena file for `input`: `input` itself if it already is one,
/// otherwise the result of converting it into `outdir`.
pub fn prepare_graphs(input: &Path, outdir: &Path) -> Result<PathBuf> {
    if is_arena_file(input)? {
        info!("INPUT {} is already converted", input.display());
        return Ok(input.to_path_buf());
    }

    create_dir_all(outdir).with_context(|| format!("failed to create {}", outdir.display()))?;
    let output = outdir.join("graphs").with_extension(EXTENSION);
    let n = convert_graphs(input, &output)?;
    info!("CONVERTED {} graphs to {}", n, output.display());
    Ok(output)
}

/// Converts every graph of a .bpcsr file into the arena format. Returns the graph count.
pub fn convert_graphs(input: &Path, output: &Path) -> Result<usize> {
    let mut reader = BufReader::new(
        File::open(input).with_context(|| format!("opening {}", input.display()))?,
    );
    let mut writer = BufWriter::new(
        File::create(output).with_context(|| format!("creating {}", output.display()))?,
    );

    write_header(&mut writer)?;

    let mut scratch = Vec::new();
    let mut total = 0usize;
    loop {
        let num_acc = match reader.read_u32::<BigEndian>() {
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(anyhow!("Failed to read graph count: {}", e)),
        };

        let graph = read_single_graph(num_acc, &mut reader)
            .with_context(|| format!("converting graph {total}"))?;
        write_graph(&mut writer, &graph, &mut scratch)?;
        total += 1;
    }

    writer.flush()?;
    Ok(total)
}
