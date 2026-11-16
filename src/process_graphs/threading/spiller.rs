//! Spill files of one writer: entries evicted from its dedup table, sharded by hash, one
//! set of files per interval (`tmp/i{interval}/w{writer}/shard_{s}.zst`).

use std::{
    fs::{File, create_dir_all},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use zstd::stream::write::Encoder;

use crate::{
    process_graphs::io::bin_writer::open_writer,
    shared::{DiskBuffer, Entry},
};

/// zstd level of the temporary spill files: written once, read once, so speed beats size.
const ENCODER_LEVEL: i32 = 1;
/// Smallest spill buffer, so tiny memory budgets still make progress.
pub const MIN_SPILL_BUF: usize = 4 * 1024;

/// Shard file with a pending buffer.
struct Shard {
    encoder: Encoder<'static, BufWriter<File>>,
    buf: DiskBuffer,
}

/// Spill files of one writer for the current interval, opened on first use.
pub struct Spiller {
    tmp_dir: PathBuf,
    writer_idx: usize,
    n_writers: usize,
    n_shards: usize,
    buf_size: usize,
    acc_width: usize,
    interval: usize,
    shards: Option<Vec<Shard>>,
}

impl Spiller {
    pub fn new(
        tmp_dir: &Path,
        writer_idx: usize,
        n_writers: usize,
        n_shards: usize,
        buf_size: usize,
        acc_width: usize,
    ) -> Self {
        Self {
            tmp_dir: tmp_dir.to_path_buf(),
            writer_idx,
            n_writers,
            n_shards: n_shards.max(1),
            buf_size: buf_size.max(MIN_SPILL_BUF),
            acc_width,
            interval: 0,
            shards: None,
        }
    }

    /// Directory of the spill files of `interval` (all writers).
    pub fn interval_dir(tmp_dir: &Path, interval: usize) -> PathBuf {
        tmp_dir.join(format!("i{interval}"))
    }

    fn open(&mut self) -> Result<&mut Vec<Shard>> {
        if self.shards.is_none() {
            let dir = Self::interval_dir(&self.tmp_dir, self.interval)
                .join(format!("w{}", self.writer_idx));
            create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            let shards = (0..self.n_shards)
                .map(|s| {
                    let file = open_writer(&dir.join(format!("shard_{s:x}.zst")))?;
                    Ok(Shard {
                        encoder: Encoder::new(file, ENCODER_LEVEL)?,
                        buf: DiskBuffer::with_capacity(self.buf_size),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            self.shards = Some(shards);
        }
        Ok(self.shards.as_mut().expect("just opened"))
    }

    /// Writes an entry to the shard of its hash. All copies of a sequence of this
    /// interval end up in the same shard file.
    pub fn write(&mut self, hash: u64, entry: &Entry) -> Result<()> {
        let (w, n_writers, n_shards, buf_size) = (
            self.acc_width,
            self.n_writers as u64,
            self.n_shards as u64,
            self.buf_size,
        );
        let shard = &mut self.open()?[((hash / n_writers) % n_shards) as usize];
        shard.buf.push(&entry.seq, &entry.accs, w)?;
        if shard.buf.len() >= buf_size {
            shard.encoder.write_all(shard.buf.as_bytes())?;
            shard.buf.clear();
        }
        Ok(())
    }

    /// Closes the shard files of the current interval. Returns whether any were written.
    pub fn finish_interval(&mut self) -> Result<bool> {
        self.interval += 1;
        let Some(shards) = self.shards.take() else {
            return Ok(false);
        };
        for mut shard in shards {
            shard.encoder.write_all(shard.buf.as_bytes())?;
            shard.encoder.finish()?.flush()?;
        }
        Ok(true)
    }
}
