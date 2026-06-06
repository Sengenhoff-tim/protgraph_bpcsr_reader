use std::path::{PathBuf};

use anyhow::{Result};
use crossbeam_channel::Sender;

use std::{
    fs,
    thread::{self, JoinHandle},
};

pub fn spawn_dispatcher(
    indir: PathBuf,
    tx_file: Sender<PathBuf>,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || -> Result<()> {
        for entry in fs::read_dir(indir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                tx_file.send(path)?;
            }
        }

        Ok(())
    })
}