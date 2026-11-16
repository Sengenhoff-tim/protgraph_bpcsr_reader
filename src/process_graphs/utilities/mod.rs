/// utility types used for graph processing
pub mod aa_pack;
mod context;
pub mod interval;
mod messages;
mod pass;
mod path_state;

pub use aa_pack::{encode_residue, pack_codes_into, unpack_into};
pub use context::TraversalContext;
pub use interval::{Interval, IntervalVecExt, WEIGHT_FACTOR};
pub use messages::{GraphMsg, TraversalJob};
pub use pass::Pass;
pub use path_state::PathState;
