//! Accession ids are stored as `w`-byte little-endian integers, where `w` is the
//! smallest width that fits every id of the run (1..=4). Lists of ids are plain byte
//! slices of `n * w` bytes and are used unchanged in memory, in shard files and in
//! the metadata output.

/// Smallest byte width that can represent ids `0..n_accessions`.
pub fn acc_width(n_accessions: usize) -> usize {
    let max = n_accessions.saturating_sub(1) as u64;
    ((64 - max.leading_zeros() as usize).div_ceil(8)).max(1)
}

/// Little-endian bytes of `id`; the first `w` bytes are the encoded id.
#[inline]
pub fn encode_acc(id: u32) -> [u8; 4] {
    id.to_le_bytes()
}

/// Decodes a list of `w`-byte ids.
pub fn decode_accs(bytes: &[u8], w: usize) -> impl Iterator<Item = u32> + '_ {
    bytes.chunks_exact(w).map(|c| {
        let mut b = [0u8; 4];
        b[..c.len()].copy_from_slice(c);
        u32::from_le_bytes(b)
    })
}

/// Sorts a list of `w`-byte ids and removes repeats, in place.
pub fn dedup_accs(accs: &mut Vec<u8>, w: usize) {
    let mut ids: Vec<[u8; 4]> = accs
        .chunks_exact(w)
        .map(|c| {
            let mut b = [0u8; 4];
            b[..w].copy_from_slice(c);
            b
        })
        .collect();
    ids.sort_unstable_by_key(|b| u32::from_le_bytes(*b));
    ids.dedup();
    accs.clear();
    for id in &ids {
        accs.extend_from_slice(&id[..w]);
    }
}
