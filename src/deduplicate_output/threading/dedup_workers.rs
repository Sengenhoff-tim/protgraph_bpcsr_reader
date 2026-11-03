use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};

use crate::shared::DiskBuffer;

pub struct WorkerResult {
    pub buffer: DiskBuffer,
    pub groups: Vec<Group>,
}

pub struct Group {
    pub seq_start: usize,
    pub seq_len: usize,
    pub metas: Vec<(Vec<u8>, u16)>,
}

pub fn spawn_worker(
    rx_paths: Receiver<PathBuf>,
    tx_entry: Sender<WorkerResult>,
    rx_buffer_empty: Receiver<DiskBuffer>,
) -> std::thread::JoinHandle<Result<()>> {
    std::thread::spawn(move || -> Result<()> {
        for path in rx_paths {
            let mut buffer = rx_buffer_empty.recv()?;

            let file = std::fs::File::open(&path)?;
            let mut decoder = zstd::stream::read::Decoder::new(file)?;
            buffer.read_from(&mut decoder)?;

            let result = build_worker_result(buffer);
            tx_entry.send(result)?;
        }
        Ok(())
    })
}

pub fn build_worker_result(buffer: DiskBuffer) -> WorkerResult {
    let base = buffer.as_bytes().as_ptr() as usize;

    let mut map: HashMap<&[u8], Vec<(Vec<u8>, u16)>> = HashMap::new();
    for record in buffer.iter() {
        let metas = record
            .metas()
            .map(|(id, pos)| (trim_id(id).to_vec(), pos));
        map.entry(record.sequence()).or_default().extend(metas);
    }

    let groups = map
        .into_iter()
        .map(|(seq, metas)| Group {
            seq_start: seq.as_ptr() as usize - base,
            seq_len: seq.len(),
            metas,
        })
        .collect();

    WorkerResult { buffer, groups }
}

fn trim_id(id: &[u8]) -> &[u8] {
    let end = id.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    &id[..end]
}