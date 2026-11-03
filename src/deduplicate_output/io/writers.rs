use std::io::Write;

use anyhow::Result;

use crate::process_graphs::utilities::unpack_into;

pub fn write_sequences<W: Write>(
    writer: &mut W,
    id: u128,
    id_offset: u128,
    packed: &[u8],
    scratch: &mut Vec<u8>,
) -> Result<()> {
    unpack_into(scratch, packed.iter().copied());

    writeln!(writer, ">pg|{}:{}", id, id_offset)?;

    for chunk in scratch.chunks(60) {
        writer.write_all(chunk)?;
        writer.write_all(b"\n")?;
    }

    Ok(())
}

/// Writes one meta as its raw id (trailing zero padding removed) followed by
/// its position as a little-endian u16.
pub fn write_meta<W: Write>(writer: &mut W, id: &[u8], pos: u16) -> Result<()> {
    let end = id.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    writer.write_all(&id[..end])?;
    writer.write_all(&pos.to_le_bytes())?;
    Ok(())
}