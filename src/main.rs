use anyhow::Result;
use mimalloc::MiMalloc;
use tracing::info;

use bpcsr_to_fasta::{
    convert_graphs::prepare_graphs,
    parameters::Config,
    pipeline::{RunParams, run},
    shared::{LoggingConfig, init_logging},
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Reads BPCSR files produced by ProtGraph and generates:
/// - a deduplicated `peptides.fasta`
/// - `metadata.bin`, the accession ids of each peptide
/// - `accessions.csv`, mapping accession ids to accessions
fn main() -> Result<()> {
    let _guard = init_logging(LoggingConfig::default())?;

    let config = Config::new()?;
    let params = RunParams::from_cli(&config.cli);

    info!("CONVERTING graphs");
    // convert .bpcsr input once into arena layout (no-op if input already is converted)
    let graph_path = prepare_graphs(&config.cli.graph_input_path, &params.outdir)?;

    info!("PROCESSING {} intervals", config.intervals.len());
    let report = run(&params, &config.intervals, &graph_path)?;
    info!(
        "DONE: {} intervals ({} spilled), {} peptides with {} accession entries",
        report.intervals,
        report.spilled_intervals,
        report.output.peptides,
        report.output.accessions
    );

    Ok(())
}
