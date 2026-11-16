use std::{path::Path, sync::Arc};

use anyhow::Result;
use tracing::info;

use super::{RunParams, RunReport};
use crate::{
    deduplicate_output::{OutputSink, dedup_spilled, write_accessions},
    process_graphs::{
        IntervalReport, ProcessPipeline, io::arena_io::ArenaFile, utilities::Interval,
    },
    shared::acc_width,
};

/// Runs all intervals over the converted graphs at `graph_path` and writes the output.
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

    let mut spilled = Vec::new();
    for (idx, &interval) in intervals.iter().enumerate() {
        let report = pipeline.run_interval(idx, interval)?;
        log_interval(idx, interval, &report);
        if report.spilled {
            spilled.push(pipeline.interval_dir(idx));
        }
    }
    pipeline.shutdown()?;

    if !spilled.is_empty() {
        info!("DEDUP of {} spilled intervals", spilled.len());
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
        spilled_intervals: spilled.len(),
        output,
    })
}

fn log_interval(idx: usize, interval: Interval, report: &IntervalReport) {
    info!(
        "INTERVAL {idx} ({interval}): {} paths, {} unique in memory, {} evicted, {} written directly{}",
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
