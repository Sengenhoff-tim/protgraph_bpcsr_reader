//! Big-endian primitives of the .bpcsr format.

use std::io::{self, BufRead, Read};

use anyhow::{Result, ensure};
use byteorder::{BigEndian, ReadBytesExt};

pub fn read_bytes<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; count];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn read_u32_vec<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<u32>> {
    let mut out = vec![0u32; count];
    reader.read_u32_into::<BigEndian>(&mut out)?;
    Ok(out)
}

pub fn read_i64_vec<R: BufRead>(reader: &mut R, count: usize) -> Result<Vec<i64>> {
    let mut out = vec![0i64; count];
    reader.read_i64_into::<BigEndian>(&mut out)?;
    Ok(out)
}

pub fn skip_bytes<R: BufRead>(reader: &mut R, count: usize) -> Result<()> {
    let skipped = io::copy(&mut reader.by_ref().take(count as u64), &mut io::sink())?;
    ensure!(skipped == count as u64, "unexpected end of file");
    Ok(())
}

/// Reads a NUL-terminated string into `buf` (without the NUL).
pub fn read_cstring_into<R: BufRead>(reader: &mut R, buf: &mut Vec<u8>) -> Result<()> {
    buf.clear();
    reader.read_until(0u8, buf)?;
    if matches!(buf.last(), Some(0)) {
        buf.pop();
    }
    Ok(())
}
