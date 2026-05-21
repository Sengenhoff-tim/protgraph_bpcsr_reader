use anyhow::Result;

mod deduplicate_output;
mod parameters;
mod process_graphs;
mod shared;

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
    let channel_dedup_in_size = config.cli.ch_proc_in_size.unwrap_or(2);
    let channel_dedup_out_size = config
        .cli
        .ch_proc_out_size
        .unwrap_or(config.cli.avail_processors * 2);

    // read graphs and produce intermediate files
    let tmp_files = process_graphs(config)?;

    // read intermediate files and write deduplicated output
    dedup_bin_files(
        tmp_files,
        avail_processors,
        &outdir_path,
        zip,
        channel_dedup_in_size,
        channel_dedup_out_size,
    )?;

    Ok(())
}
