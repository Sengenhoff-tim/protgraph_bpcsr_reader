use std::io::{BufRead, ErrorKind};

use anyhow::{Result, anyhow};
use byteorder::{BigEndian, ReadBytesExt};
use crossbeam_channel::Sender;

use crate::process_graphs::{graph::ProteinGraph, io::bpcsr_reader::read_single_graph};

pub fn spawn_protein_graph_reader<R: BufRead>(rdr: R, tx_protgraph: Sender<Result<ProteinGraph>>) {
    let mut rdr = rdr;

    loop {
        let num_acc = match rdr.read_u32::<BigEndian>() {
            Ok(n) => n,
            Err(e) => {
                if e.kind() != ErrorKind::UnexpectedEof {
                    let _ = tx_protgraph.send(Err(anyhow!("{}", e)));
                }
                break;
            }
        };

        match read_single_graph(num_acc, &mut rdr) {
            Ok(pg) => {
                if tx_protgraph.send(Ok(pg)).is_err() {
                    break;
                }
            }
            Err(e) => {
                let _ = tx_protgraph.send(Err(e));
                break;
            }
        }
    }
}
