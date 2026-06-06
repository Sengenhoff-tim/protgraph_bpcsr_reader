use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use crate::shared::{EntryBuffer, bin_entry::{MetaRef, SeqRef}};

pub struct WorkerResult {
    pub buffer: EntryBuffer,
    pub groups: Vec<(SeqRef, Vec<MetaRef>)>,
}

pub fn spawn_worker(
    rx_paths: Receiver<PathBuf>,
    tx_entry: Sender<WorkerResult>,
    rx_buffer_empty: Receiver<EntryBuffer>,
) -> std::thread::JoinHandle<Result<()>> {
    std::thread::spawn(move || -> Result<()> {

        for path in rx_paths {
            let mut buffer = rx_buffer_empty.recv()?;

            buffer.data = std::fs::read(path)?;

            let result = build_worker_result(buffer);

            tx_entry.send(result)?;
        }

        Ok(())
    })
}

pub fn build_worker_result(buffer: EntryBuffer) -> WorkerResult {
    let mut map: HashMap<&[u8], (SeqRef,Vec<MetaRef>)> = HashMap::new();

    for entry in buffer.iter() {
        let (seq, meta) = entry.get_seq_meta_ref(&buffer.data);

        let seq_bytes = seq.get(&buffer);

        map.entry(seq_bytes)
            .and_modify(|(_, metas)| {
                metas.push(meta);
            })
            .or_insert_with(|| {
                (seq, vec![meta])
            });
    }

    let groups = map.into_values().collect();

    WorkerResult {
        buffer,
        groups,
    }
}