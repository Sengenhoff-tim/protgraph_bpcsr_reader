use std::collections::HashMap;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};

use crate::shared::{BinEntry, BinEntryMeta};

pub fn spawn_worker(
    rx: Receiver<Vec<BinEntry>>,
    tx_out: Sender<(String, Vec<BinEntryMeta>)>,
) -> std::thread::JoinHandle<Result<()>> {
    std::thread::spawn(move || -> Result<()> {
        while let Ok(shard) = rx.recv() {
            process_shard(shard, &tx_out)?;
        }

        Ok(())
    })
}

fn process_shard(
    entries: Vec<BinEntry>,
    tx_out: &Sender<(String, Vec<BinEntryMeta>)>,
) -> Result<()> {
    let mut unique_entries: HashMap<String, Vec<BinEntryMeta>> = HashMap::new();

    for e in entries {
        unique_entries.entry(e.seq).or_default().push(e.meta);
    }

    for (key, metas) in unique_entries {
        tx_out.send((key, metas))?;
    }

    Ok(())
}
