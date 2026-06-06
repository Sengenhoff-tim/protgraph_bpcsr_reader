/// A continuous vector of character bytes for string deduplication.
/// Strings are accessable by index.
/// This closely aligns with the original implementation.
use std::{
    collections::{HashMap, hash_map::Entry},
    str,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StringRef {
    pub start: u32,
    pub len: u32,
}

#[derive(Clone)]
pub struct StringTable {
    buffer: Vec<u8>,
    mapping: Vec<StringRef>,
}

impl StringTable {
    #[inline(always)]
    pub fn get_str(&self, idx: usize) -> &str {
        let r = unsafe { *self.mapping.get_unchecked(idx) };

        unsafe {
            std::str::from_utf8_unchecked(
                self.buffer
                    .get_unchecked(r.start as usize..r.start as usize + r.len as usize),
            )
        }
    }

    pub fn get_slice(&self, idx: usize) -> &[u8] {
        let r = unsafe { self.mapping.get_unchecked(idx) };

        unsafe {
            self.buffer
                .get_unchecked(r.start as usize..(r.start + r.len) as usize)
        }
    }

    /// Struct is populated from input string
    pub fn build_from_strings<I>(items: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let mut buffer = Vec::new();
        let mut mapping = Vec::new();

        // HashMap for dedup
        let mut map: HashMap<Box<[u8]>, (StringRef, u32)> = HashMap::new();

        let iter = items.into_iter();
        let (lower, _) = iter.size_hint();
        mapping.reserve(lower);

        for s in iter {
            let bytes = s.into_bytes();

            match map.entry(bytes.clone().into_boxed_slice()) {
                Entry::Occupied(e) => {
                    let (r, _idx) = *e.get();
                    mapping.push(r);
                }
                Entry::Vacant(e) => {
                    let start = buffer.len() as u32;
                    let len = bytes.len() as u32;

                    buffer.extend_from_slice(&bytes);

                    let r = StringRef { start, len };
                    let idx = mapping.len() as u32;

                    e.insert((r, idx));
                    mapping.push(r);
                }
            }
        }

        Self { buffer, mapping }
    }
}
