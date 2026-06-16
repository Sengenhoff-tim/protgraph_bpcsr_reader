use std::io::Write;

use anyhow::Result;

pub fn write_sequences<W: Write>(writer: &mut W, id: u128, sequence: &[u8]) -> Result<()> {
    writeln!(writer, ">pg|{}", id)?;

    for chunk in sequence.chunks(60) {
        writer.write_all(chunk)?;
        writer.write_all(b"\n")?;
    }

    Ok(())
}

pub fn write_meta<W: Write>(writer: &mut W, id: u128, meta: &[u8]) -> Result<()> {
    let mut pos = 0;

    let acc_len = u32::from_le_bytes(meta[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let accession = &meta[pos..pos + acc_len];
    pos += acc_len;

    let qual_len = u32::from_le_bytes(meta[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let qualifiers = &meta[pos..pos + qual_len];
    pos += qual_len;

    let spos = u16::from_le_bytes(meta[pos..pos + 2].try_into().unwrap());
    pos += 2;

    let epos = u16::from_le_bytes(meta[pos..pos + 2].try_into().unwrap());
    pos += 2;

    let mssclvg = u16::from_le_bytes(meta[pos..pos + 2].try_into().unwrap());

    write!(writer, "{},", id)?;
    writer.write_all(accession)?;
    write!(writer, ",")?;

    match spos {
        u16::MAX => write!(writer, "?,")?,
        v => write!(writer, "{},", v)?,
    }

    match epos {
        u16::MAX => write!(writer, "?,")?,
        v => write!(writer, "{},", v)?,
    }

    write!(writer, "{},[", mssclvg)?;

    let mut first = true;
    for part in qualifiers.split(|&b| b == b',') {
        if !first { writer.write_all(b"|")?; }
        writer.write_all(part)?;
        first = false;
    }

    writeln!(writer, "]")?;

    Ok(())
}
