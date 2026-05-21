use bincode::{Decode, Encode};

#[derive(Encode, Decode, Debug)]
pub struct BinEntry {
    pub seq: String,
    pub meta: BinEntryMeta,
}

#[derive(Encode, Decode, Debug)]
pub struct BinEntryMeta {
    pub acc: String,
    pub qualifiers: String,
    pub spos: Option<u16>,
    pub epos: Option<u16>,
    pub mssclvg: u32,
}
