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
