pub struct ChunkBuffer {
    pub data: Vec<u8>,
}

impl ChunkBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        ChunkBuffer { data: Vec::with_capacity(cap) }
    }
    pub fn clear(&mut self) {
        self.data.clear();
    }
}