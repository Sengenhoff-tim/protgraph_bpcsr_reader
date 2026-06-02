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

        

        let mut len_buf: [u8; 4] = [0u8; 4];

        let mut entry_buf: Vec<u8> = Vec::new();
        
        for path in result {
            let entries = read_entries_binary(&path, &mut len_buf, &mut entry_buf)
                .with_context(|| format!("failed to read entries from {}", path.display()))?;

            tx.send(entries).context("failed to send decoded entries")?;
        }

        Ok(())
    })
}
