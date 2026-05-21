use std::path::PathBuf;

use anyhow::{Context, Result};
use crossbeam_channel::Sender;

use crate::deduplicate_output::io::read_entries_binary;
use crate::shared::BinEntry;

pub fn spawn_dispatcher(
    result: Vec<PathBuf>,
    tx: Sender<Vec<BinEntry>>,
) -> std::thread::JoinHandle<Result<()>> {
    std::thread::spawn(move || -> Result<()> {
        for path in result {
            let entries = read_entries_binary(&path)
                .with_context(|| format!("failed to read entries from {}", path.display()))?;

            tx.send(entries).context("failed to send decoded entries")?;
        }

        Ok(())
    })
}
