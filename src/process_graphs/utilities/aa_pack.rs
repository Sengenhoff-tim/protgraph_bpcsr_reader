/// Code 0 is the filler. Residues use codes 1..=20.
const FILLER: u8 = 0;
const INVALID: u8 = u8::MAX;

/// Index = 5-bit code. 0 is the filler and is never emitted as a residue.
const AA_ALPHABET: &[u8; 27] = b"\0ACDEFGHIKLMNPQRSTVWYBZXUOJ";

/// Maps an ASCII byte to its 5-bit code. Anything else maps to INVALID.
const CODE: [u8; 256] = {
    let mut table = [INVALID; 256];
    let mut i = 1;
    while i < AA_ALPHABET.len() {
        table[AA_ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    table
};

/// Maps an ASCII residue to its 5-bit code, `None` for anything not in the alphabet.
#[inline]
pub fn encode_residue(c: u8) -> Option<u8> {
    let code = CODE[c as usize];
    (code != INVALID).then_some(code)
}

/// Packs 5-bit residue codes (as produced by [`encode_residue`]) into `buf`.
/// `buf` is cleared first. The last byte is padded with filler bits.
pub fn pack_codes_into<I>(buf: &mut Vec<u8>, codes: I)
where
    I: IntoIterator<Item = u8>,
{
    buf.clear();

    let mut acc: u32 = 0;
    let mut bits: u32 = 0;

    for code in codes {
        acc = (acc << 5) | code as u32;
        bits += 5;

        if bits >= 8 {
            bits -= 8;
            buf.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    if bits > 0 {
        buf.push((acc << (8 - bits)) as u8);
    }
}

/// Unpacks 5-bit codes from an iterator of packed bytes, pushing one-letter codes into `buf`.
/// `buf` is cleared first. Stops at the first filler code or when the input is exhausted.
pub fn unpack_into<I>(buf: &mut Vec<u8>, packed: I)
where
    I: IntoIterator<Item = u8>,
{
    buf.clear();

    let mut acc: u32 = 0;
    let mut bits: u32 = 0;

    for byte in packed {
        acc = (acc << 8) | byte as u32;
        bits += 8;

        while bits >= 5 {
            bits -= 5;
            let code = ((acc >> bits) & 0x1F) as u8;
            if code == FILLER {
                return;
            }
            buf.push(AA_ALPHABET[code as usize]);
        }

        acc &= (1 << bits) - 1;
    }
}
