use std::path::PathBuf;

use anyhow::Result;
use crossbeam_channel::Sender;
use tracing::info;

use std::{
    fs,
    thread::{self, JoinHandle},
};

pub fn spawn_dispatcher(indir: PathBuf, tx_file: Sender<PathBuf>) -> JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        let mut sent = 0usize;
        for dir in fs::read_dir(&indir)? {
            let dir = dir?.path();
            if !dir.is_dir() {
                continue;
            }
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.is_file() && path.extension().is_some_and(|e| e == "zst") {
                    tx_file.send(path)?;
                    sent += 1;
                }
            }
        }
        info!("DISPATCHER sent {sent} shards");
        Ok(())
    })
}