use anyhow::Result;

mod deduplicate_output;
mod parameters;
mod process_graphs;
mod shared;

use mimalloc::MiMalloc;

#[cfg(test)]
mod tests;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use crate::{
    deduplicate_output::dedup_bin_files, parameters::Config, process_graphs::process_graphs,
};

/// Reads BPCSR files produced by ProtGraph and generates:
/// - a deduplicated `peptides.fasta`
/// - `metadata.csv`, describing which proteins generated each peptide
/// - `log.csv`, containing run information
fn main() -> Result<()> {
    let config = Config::new()?;

    let avail_processors = config.cli.avail_processors;
    let outdir_path = config.cli.outdir_path.clone();
    let zip = config.cli.zip;

    let memory = config.cli.avail_memory;

    // read graphs and produce intermediate files
    process_graphs(config)?;

    // read intermediate files and write deduplicated output
    let tmp_path = outdir_path.join("tmp");

    let has_input_files = tmp_path
        .read_dir()
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false); // tmp doesn't exist -> treat as "no files"

    if has_input_files {
        dedup_bin_files(
            avail_processors as u64,
            memory as u64,
            &outdir_path,
            tmp_path,
            zip,
        )?;
    }

    Ok(())
}
