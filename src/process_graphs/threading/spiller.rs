//! Spill files of one writer: entries evicted from its dedup table, sharded by bucket, one
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
    shared::{Entry, write_record},
};

/// zstd level of the temporary spill files: written once, read once, so speed beats size.
const ENCODER_LEVEL: i32 = 1;
/// A shard file. Records go straight into the zstd encoder, which buffers its input
/// and compresses in blocks; the file below is buffered as well.
type Shard = Encoder<'static, BufWriter<File>>;

/// Spill files of one writer for the current interval, opened on first use.
pub struct Spiller {
    tmp_dir: PathBuf,
    writer_idx: usize,
    n_shards: usize,
    acc_width: usize,
    interval: usize,
    shards: Option<Vec<Shard>>,
}

impl Spiller {
    pub fn new(tmp_dir: &Path, writer_idx: usize, n_shards: usize, acc_width: usize) -> Self {
        Self {
            tmp_dir: tmp_dir.to_path_buf(),
            writer_idx,
            n_shards: n_shards.max(1),
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
                    Ok(Encoder::new(file, ENCODER_LEVEL)?)
                })
                .collect::<Result<Vec<_>>>()?;
            self.shards = Some(shards);
        }
        Ok(self.shards.as_mut().expect("just opened"))
    }

    /// Writes an entry of dedup table bucket `bucket` to that bucket's shard. All copies
    /// of a sequence share a bucket, so they end up in the same shard file.
    pub fn write(&mut self, bucket: usize, entry: &Entry) -> Result<()> {
        let (w, n_shards) = (self.acc_width, self.n_shards);
        let shard = &mut self.open()?[bucket % n_shards];
        write_record(shard, &entry.seq, &entry.accs, w)
    }

    /// Closes the shard files of the current interval. Returns whether any were written.
    pub fn finish_interval(&mut self) -> Result<bool> {
        self.interval += 1;
        let Some(shards) = self.shards.take() else {
            return Ok(false);
        };
        for shard in shards {
            shard.finish()?.flush()?;
        }
        Ok(true)
    }
}
