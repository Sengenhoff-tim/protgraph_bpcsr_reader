use std::io::Write;

use anyhow::Result;

use crate::{process_graphs::utilities::unpack_into, shared::bin_entry::META_LEN};

pub fn write_sequences<W: Write>(
    writer: &mut W,
    id: u128,
    id_start: u128,
    packed: &[u8],
    scratch: &mut Vec<u8>,
) -> Result<()> {
    unpack_into(scratch, packed.iter().copied());

    writeln!(writer, ">pg|{}:{}", id, id_start)?;

    for chunk in scratch.chunks(60) {
        writer.write_all(chunk)?;
        writer.write_all(b"\n")?;
    }

    Ok(())
}

pub fn write_meta<W: Write>(writer: &mut W, meta: &[u8]) -> Result<()> {
    // meta = ID (ID_LEN bytes, zero-padded) followed by start position (START_LEN bytes, LE)
    writer.write_all(&meta[..META_LEN])?;
    Ok(())
}