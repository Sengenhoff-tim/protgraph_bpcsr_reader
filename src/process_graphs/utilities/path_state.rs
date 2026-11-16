/// State after stepping onto a node; a path is a list of these.
#[derive(Debug, Clone, Copy)]
pub struct PathState {
    /// word offset of the node block in the graph arena
    pub node: u32,
    pub var: u16,
    pub cleaves: u16,
    pub tv: i64,
}
