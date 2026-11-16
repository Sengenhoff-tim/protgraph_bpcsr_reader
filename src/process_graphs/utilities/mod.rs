pub mod aa_pack;
/// utility stucts used for graph processing
pub mod interval;
pub mod types;

pub use aa_pack::{encode_residue, pack_codes_into, unpack_into};
pub use interval::{Interval, IntervalVecExt, WEIGHT_FACTOR};
pub use types::{PathState, TraversalContext, TraversalJob};
