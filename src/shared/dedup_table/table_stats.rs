#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TableStats {
    pub inserted: u64,
    pub hits: u64,
    pub evicted: u64,
}
