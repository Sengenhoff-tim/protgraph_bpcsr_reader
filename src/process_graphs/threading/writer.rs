//! Writer threads: each owns the dedup table for its share of the hash space.
//!
//! Records are deduplicated in memory; entries evicted from the table go to the
//! [`Spiller`]. At the end of an interval, entries of buckets that never evicted are
//! complete and go straight to the output; entries of evicting buckets are spilled as
//! well, so the final shard dedup can merge them with their evicted copies.

use std::mem;

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use tracing::info;

use super::{Spiller, WriterMsg, WriterReport};
use crate::shared::{DedupTable, MemBuffer, OutputBatch, TableStats};

/// Size of the output batches sent at the end of an interval.
const OUTPUT_BATCH_BYTES: usize = 1 << 20;

pub struct Writer {
    pub idx: usize,
    pub table: DedupTable,
    pub spiller: Spiller,
    /// returns emptied batch buffers to the workers
    pub tx_reuse: Sender<MemBuffer>,
    pub tx_out: Sender<OutputBatch>,
    /// table stats at the end of the previous interval
    pub stats_before: TableStats,
}

impl Writer {
    pub fn run(mut self, rx: Receiver<WriterMsg>) -> Result<()> {
        for msg in rx {
            match msg {
                WriterMsg::Batch(batch) => self.insert(batch)?,
                WriterMsg::EndInterval(ack) => {
                    let report = self.finish_interval()?;
                    ack.send(report).context("coordinator gone")?;
                }
            }
        }
        Ok(())
    }

    fn insert(&mut self, mut batch: MemBuffer) -> Result<()> {
        let spiller = &mut self.spiller;
        for rec in batch.iter() {
            self.table
                .insert(rec.hash, rec.seq, rec.acc, &mut |hash, entry| {
                    spiller.write(hash, &entry)
                })?;
        }
        batch.clear();
        // the pool may already be closed during shutdown
        let _ = self.tx_reuse.send(batch);
        Ok(())
    }

    /// Drains the table: complete entries to the output, the rest to the spill files.
    fn finish_interval(&mut self) -> Result<WriterReport> {
        let (spiller, tx_out) = (&mut self.spiller, &self.tx_out);
        let mut out = OutputBatch::default();
        let mut direct = 0u64;
        self.table.drain(|hash, entry, evicted| {
            if evicted {
                return spiller.write(hash, &entry);
            }
            out.push(&entry.seq, &entry.accs);
            direct += 1;
            if out.byte_len() >= OUTPUT_BATCH_BYTES {
                tx_out.send(mem::take(&mut out)).context("output closed")?;
            }
            Ok(())
        })?;
        if !out.is_empty() {
            tx_out.send(out).context("output closed")?;
        }
        let spilled = self.spiller.finish_interval()?;

        let stats = self.interval_stats();
        info!(
            "writer {}: {} new, {} duplicates, {} evicted, {} direct",
            self.idx, stats.inserted, stats.hits, stats.evicted, direct
        );
        Ok(WriterReport {
            spilled,
            direct,
            stats,
        })
    }

    /// Table stats since the previous interval.
    fn interval_stats(&mut self) -> TableStats {
        let total = self.table.stats();
        let before = mem::replace(&mut self.stats_before, total);
        TableStats {
            inserted: total.inserted - before.inserted,
            hits: total.hits - before.hits,
            evicted: total.evicted - before.evicted,
        }
    }
}
