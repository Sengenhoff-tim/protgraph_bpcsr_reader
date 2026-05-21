use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering::Relaxed},
};

use anyhow::Result;
use crossbeam_channel::Sender;
use rayon::{ThreadPoolBuilder, scope};

use crate::process_graphs::{
    graph::{MetaData, ProteinGraph, TraversalData},
    utilities::{Interval, TraversalStatus},
};
use crate::shared::BinEntry;

pub struct WorkerArgs {
    pub max_vars: u8,
    pub limit: usize,
    pub n_splits: u8,
    pub max_depth: u8,
}

pub fn spawn_workers(
    protein_graph: ProteinGraph,
    intervals: Arc<Vec<Interval>>,
    tx_entry: Sender<BinEntry>,
    num_threads: usize,
    incomplete: Arc<AtomicBool>,
    args: Arc<WorkerArgs>,
) -> Result<()> {
    let traversal_data = Arc::new(protein_graph.traversal_data);
    let meta_data = Arc::new(protein_graph.meta_data);

    let pool = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap();

    pool.install(|| {
        scope(|s| {
            for interval in intervals.iter().cloned() {
                let data = Arc::clone(&traversal_data);
                let meta = Arc::clone(&meta_data);
                let tx_entry = tx_entry.clone();
                let incomplete = Arc::clone(&incomplete);
                let args = Arc::clone(&args);

                s.spawn(move |_| {
                    traversal_thread(data, meta, tx_entry, interval, incomplete, 0, args);
                });
            }
        });
    });

    Ok(())
}

fn traversal_thread(
    data: Arc<TraversalData>,
    meta: Arc<MetaData>,
    tx_entry: Sender<BinEntry>,
    interval: Interval,
    incomplete: Arc<AtomicBool>,
    depth: u8,
    args: Arc<WorkerArgs>,
) {
    // depth termination
    if depth >= args.max_depth {
        incomplete.store(true, Relaxed);
        return;
    }

    match data.traverse(&interval, args.max_vars, args.limit) {
        Ok(TraversalStatus::Overflow()) => {
            let splits = interval.split_to_n(args.n_splits);

            for sub in splits {
                let data = Arc::clone(&data);
                let meta = Arc::clone(&meta);
                let tx_entry = tx_entry.clone();
                let incomplete = Arc::clone(&incomplete);
                let args = Arc::clone(&args);

                rayon::spawn(move || {
                    traversal_thread(data, meta, tx_entry, sub, incomplete, depth + 1, args);
                });
            }
        }

        Ok(TraversalStatus::Complete(state)) => {
            let final_states = &state.states_at_node[data.nodes.len() - 1];

            for &state_id in final_states {
                let trace = state.reconstruct_trace(state_id);

                if let Ok(Some(entry)) = meta.build_peptide(&trace) {
                    let _ = tx_entry.send(entry);
                }
            }
        }

        Err(e) => {
            eprintln!("traverse error: {e:?}");
        }
    }
}
