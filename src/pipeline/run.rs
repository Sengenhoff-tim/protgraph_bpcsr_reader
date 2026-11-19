use std::{path::Path, sync::Arc};

use anyhow::Result;
use tracing::info;

use super::{RunParams, RunReport};
use crate::{
    deduplicate_output::{OutputSink, dedup_spilled, write_accessions},
    process_graphs::{PassReport, ProcessPipeline, io::arena_io::ArenaFile, utilities::Interval},
    shared::acc_width,
};

/// Traverses the converted graphs at `graph_path` against all `intervals` (sorted and
/// merged) in one pass and writes the deduplicated output.
pub fn run(params: &RunParams, intervals: &[Interval], graph_path: &Path) -> Result<RunReport> {
    let arena = Arc::new(ArenaFile::open(graph_path)?);
    let w = acc_width(arena.accessions().len());
    info!(
        "{} accessions, ids stored with {} bytes",
        arena.accessions().len(),
        w
    );
    write_accessions(&params.outdir, arena.accessions(), params.zip)?;

    let tmp = params.outdir.join("tmp");
    remove_dir_if_exists(&tmp)?;

    let sink = OutputSink::spawn(&params.outdir, params.zip, w)?;
    let pipeline = ProcessPipeline::spawn(&params.process, arena, &tmp, sink.sender(), w)?;

    let report = pipeline.run_pass(0, intervals)?;
    log_pass(intervals.len(), &report);
    let spilled: Vec<_> = report
        .spilled
        .then(|| pipeline.pass_dir(0))
        .into_iter()
        .collect();
    pipeline.shutdown()?;

    if !spilled.is_empty() {
        info!("DEDUP of spilled entries");
        dedup_spilled(
            &spilled,
            params.process.threads,
            params.dedup_budget,
            w,
            sink.sender(),
        )?;
    }
    let output = sink.finish()?;
    remove_dir_if_exists(&tmp)?;

    info!(
        "OUTPUT {} peptides, {} accession entries",
        output.peptides, output.accessions
    );
    Ok(RunReport {
        intervals: intervals.len(),
        spilled: report.spilled,
        output,
    })
}

fn log_pass(n_intervals: usize, report: &PassReport) {
    info!(
        "PASS over {n_intervals} intervals: {} paths, {} unique in memory, {} evicted, {} written directly{}",
        report.stats.inserted + report.stats.hits,
        report.stats.inserted,
        report.stats.evicted,
        report.direct,
        if report.spilled { ", spilled" } else { "" }
    );
}

/// Removes `dir` if it exists.
fn remove_dir_if_exists(dir: &Path) -> Result<()> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
