use std::{io::{BufRead, ErrorKind}, thread::{self, JoinHandle}};

use anyhow::{Result, anyhow, Context};
use byteorder::{BigEndian, ReadBytesExt};
use crossbeam_channel::Sender;

use crate::process_graphs::{graph::ProteinGraph, io::bpcsr_reader::read_single_graph};

pub fn spawn_protein_graph_reader<R: BufRead + Send + 'static>(
    rdr: R,
    tx_protgraph: Sender<ProteinGraph>,
) -> JoinHandle<Result<()>> {
    thread::spawn(move || {
        let mut rdr = rdr;

        loop {
            let num_acc = match rdr.read_u32::<BigEndian>() {
                Ok(n) => n,
                Err(e) => {
                    if e.kind() != ErrorKind::UnexpectedEof {
                        return Err(anyhow!("Failed to read graph count: {}", e));
                    }
                    break;
                }
            };

            let pg = read_single_graph(num_acc, &mut rdr)?;
            tx_protgraph.send(pg).context("Channel receiver dropped")?;
        }

        Ok(())
    })
}

