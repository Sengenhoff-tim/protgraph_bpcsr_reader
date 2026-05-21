use std::io::Write;

use anyhow::Result;

use crate::shared::BinEntryMeta;

pub fn write_sequences<W: Write>(writer: &mut W, id: usize, sequence: &str) -> Result<()> {
    writeln!(writer, ">pg|{}\n{}", id, insert_newlines(sequence))?;
    Ok(())
}

pub fn write_meta<W: Write>(writer: &mut W, id: usize, metas: &[BinEntryMeta]) -> Result<()> {
    for meta in metas {
        let spos = meta
            .spos
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".to_string());
        let epos = meta
            .epos
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".to_string());
        let qualifiers = meta.qualifiers.replace(",", "|");
        writeln!(
            writer,
            "{},{},{},{},{},[{}]",
            id, meta.acc, spos, epos, meta.mssclvg, qualifiers
        )?;
    }
    Ok(())
}

fn insert_newlines(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, chunk) in chars.chunks(60).enumerate() {
        if i > 0 {
            result.push('\n');
        }
        result.extend(chunk);
    }

    result
}
