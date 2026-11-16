//! Long-lived pipeline threads whose failure must wake up the coordinator.

use std::thread::{self, JoinHandle};

use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender, select, unbounded};

/// Owns the pipeline threads. A thread that returns an error or panics reports it on an
/// error channel, so the coordinator can stop waiting instead of hanging.
pub struct Supervisor {
    err_tx: Sender<String>,
    err_rx: Receiver<String>,
    handles: Vec<JoinHandle<Result<()>>>,
}

impl Default for Supervisor {
    fn default() -> Self {
        let (err_tx, err_rx) = unbounded();
        Self {
            err_tx,
            err_rx,
            handles: Vec::new(),
        }
    }
}

impl Supervisor {
    /// Spawns `f` as a named, supervised thread.
    pub fn spawn<F>(&mut self, name: String, f: F) -> Result<()>
    where
        F: FnOnce() -> Result<()> + Send + 'static,
    {
        let err_tx = self.err_tx.clone();
        let handle = thread::Builder::new()
            .name(name.clone())
            .spawn(move || {
                let mut guard = FailGuard {
                    name,
                    tx: err_tx,
                    armed: true,
                };
                let result = f();
                if let Err(e) = &result {
                    let _ = guard.tx.send(format!("{}: {e:#}", guard.name));
                }
                guard.armed = false;
                result
            })
            .map_err(|e| anyhow!("failed to spawn thread: {e}"))?;
        self.handles.push(handle);
        Ok(())
    }

    /// Waits for `rx`, failing early if any supervised thread failed.
    pub fn wait<T>(&self, rx: &Receiver<T>) -> Result<T> {
        select! {
            recv(rx) -> v => v.map_err(|_| anyhow!("pipeline thread stopped unexpectedly")),
            recv(self.err_rx) -> e => Err(anyhow!(e.unwrap_or_else(|_| "pipeline failed".into()))),
        }
    }

    /// Joins all threads; returns the first error any of them reported.
    pub fn join(self) -> Result<()> {
        let mut first_err = None;
        for handle in self.handles {
            let result = handle
                .join()
                .map_err(|_| anyhow!("pipeline thread panicked"))
                .and_then(|r| r);
            if let Err(e) = result {
                first_err.get_or_insert(e);
            }
        }
        first_err.map_or(Ok(()), Err)
    }
}

/// Reports a thread's failure on drop unless disarmed, so panics are reported too.
struct FailGuard {
    name: String,
    tx: Sender<String>,
    armed: bool,
}

impl Drop for FailGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.tx.send(format!("{} panicked", self.name));
        }
    }
}
