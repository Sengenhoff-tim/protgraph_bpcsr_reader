use std::{
    fs::create_dir_all,
    io::Write,
    path::Path,
    thread::{self, JoinHandle},
};

use anyhow::{ensure, Error, Result};
use crossbeam_channel::{Receiver, Sender};

use tracing::info;
use zstd::stream::write::Encoder;

use crate::process_graphs::io::bin_writer::open_writer;
use crate::shared::{DiskBuffer, MemBuffer};

const ENCODER_LEVEL: i32 = 3;
/// Smallest buffer we will allocate, so tiny memory budgets still make progress.
const MIN_BUF_SIZE: usize = 4 * 1024;

/// Two pending encoded buffers per slot. `active` receives records; when it
/// reaches its target size it is written to the encoder and the other buffer
/// becomes active.
struct Slot {
    encoder: Encoder<'static, std::io::BufWriter<std::fs::File>>,
    bufs: [DiskBuffer; 2],
    active: usize,
    target: usize,
}

impl Slot {
    /// Writes one pending buffer to the encoder and clears it.
    fn drain(&mut self, i: usize) -> Result<()> {
        let buf = &mut self.bufs[i];
        if !buf.as_bytes().is_empty() {
            self.encoder.write_all(buf.as_bytes())?;
            buf.clear();
        }
        Ok(())
    }

    /// Switches the active buffer, then writes the full one out.
    fn rotate(&mut self) -> Result<()> {
        let full = self.active;
        self.active ^= 1;
        self.drain(full)
    }

    fn flush(&mut self) -> Result<()> {
        for i in 0..2 {
            self.drain(i)?;
        }
        Ok(())
    }
}

/// Opens all shard files up front and spawns one writer per channel.
/// `max_memory` (bytes) is split across every buffer in the system; each
/// buffer's target size comes from that split.
pub fn spawn_writer_managers(
    rx_filled: Vec<Receiver<MemBuffer>>,
    tx_empty: Sender<MemBuffer>,
    out_dir: &Path,
    max_handles: Option<u32>,
    max_memory: usize,
) -> Result<Vec<JoinHandle<Result<(), Error>>>> {
    ensure!(
        rx_filled.len() == tx_empty.len(),
        "rx/tx writer channel count mismatch"
    );
    let n_writers = rx_filled.len().max(1);

    let budget = max_handles.unwrap_or_else(get_sys_open_files).max(1) as usize;
    ensure!(budget >= n_writers, "handle budget smaller than writer count");

    // Total buffers = slots * 2. Each buffer gets an equal share of max_memory.
    let total_buffers = budget * 2;
    let buf_size = (max_memory / total_buffers).max(MIN_BUF_SIZE);
    info!("writer buffers: {total_buffers} x {buf_size} bytes");

    let base = budget / n_writers;
    let rem = budget % n_writers;

    let mut all_slots: Vec<Vec<Slot>> = Vec::with_capacity(n_writers);
    for idx in 0..n_writers {
        let n_slots = base + usize::from(idx < rem);
        let dir = out_dir.join("tmp").join(format!("w{idx}"));
        create_dir_all(&dir)?;

        let mut slots = Vec::with_capacity(n_slots);
        for s in 0..n_slots {
            let path = dir.join(format!("shard_{s:x}.zst"));
            let file = open_writer(&path)?;
            slots.push(Slot {
                encoder: Encoder::new(file, ENCODER_LEVEL)?,
                bufs: [
                    DiskBuffer::with_capacity(buf_size),
                    DiskBuffer::with_capacity(buf_size),
                ],
                active: 0,
                target: buf_size,
            });
        }
        all_slots.push(slots);
    }

    let handles = rx_filled
        .into_iter()
        .zip(all_slots)
        .enumerate()
        .map(|(idx, (rx, slots))| {
            let tx = tx_empty.clone();
            thread::spawn(move || writer_thread(idx, rx, tx, slots))
        })
        .collect();

    Ok(handles)
}

fn writer_thread(
    idx: usize,
    rx_filled: Receiver<MemBuffer>,
    tx_empty: Sender<MemBuffer>,
    mut slots: Vec<Slot>,
) -> Result<()> {
    let n_slots = slots.len();
    ensure!(n_slots > 0, "writer {idx} has no slots");

    while let Ok(mut record_buf) = rx_filled.recv() {
        let mut completed = 0usize;
        for record in record_buf.iter() {
            let slot = &mut slots[(record.hash as usize) % n_slots];

            let active = &mut slot.bufs[slot.active];
            active.push(record)?;
            completed += 1;

            if active.as_bytes().len() >= slot.target {
                slot.rotate()?;
            }
        }
        info!("writer {idx}: BATCH_COMPLETE for {completed} entries");
        record_buf.clear();
        let _ = tx_empty.send(record_buf);
    }

    for slot in slots.iter_mut() {
        slot.flush()?;
    }
    for slot in slots {
        slot.encoder.finish()?;
    }

    Ok(())
}

#[cfg(unix)]
fn get_sys_open_files() -> u32 {
    use nix::sys::resource::{getrlimit, Resource};

    // uses 80% of file handles
    match getrlimit(Resource::RLIMIT_NOFILE) {
        Ok((soft, _)) => (soft * 8 / 10).clamp(64, 8192) as u32,
        Err(_) => 512,
    }
}

#[cfg(windows)]
fn get_sys_open_files() -> u32 {
    2048
}