//! Deduplicated peptides on their way to the output files.

/// Sequences with their complete accession id lists. Every sequence in a batch is
/// final: no other batch of the run contains it.
#[derive(Debug, Default)]
pub struct OutputBatch {
    seqs: Vec<u8>,
    accs: Vec<u8>,
    /// (sequence length, accession byte length) per peptide
    groups: Vec<(u32, u32)>,
}

impl OutputBatch {
    pub fn push(&mut self, seq: &[u8], accs: &[u8]) {
        self.seqs.extend_from_slice(seq);
        self.accs.extend_from_slice(accs);
        self.groups.push((seq.len() as u32, accs.len() as u32));
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn byte_len(&self) -> usize {
        self.seqs.len() + self.accs.len() + self.groups.len() * 8
    }

    /// Yields (packed sequence, accession id bytes).
    pub fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        let (mut s, mut a) = (0usize, 0usize);
        self.groups.iter().map(move |&(sl, al)| {
            let seq = &self.seqs[s..s + sl as usize];
            let accs = &self.accs[a..a + al as usize];
            s += sl as usize;
            a += al as usize;
            (seq, accs)
        })
    }
}
