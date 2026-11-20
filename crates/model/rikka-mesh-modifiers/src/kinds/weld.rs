use crate::codec::{non_negative, record, tagged};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WeldMode {
    #[default]
    All,
    Connected,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Weld {
    pub mode: WeldMode,
    pub merge_threshold: f32,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
    pub loose_edges: bool,
}

impl Default for Weld {
    fn default() -> Weld {
        Weld {
            mode: WeldMode::All,
            merge_threshold: 0.001,
            vertex_group: String::new(),
            invert_vertex_group: false,
            loose_edges: false,
        }
    }
}

tagged!(WeldMode, "weld mode" {
    All = 0,
    Connected = 1,
});

record!(Weld {
    mode,
    merge_threshold => non_negative("weld merge threshold is negative"),
    vertex_group,
    invert_vertex_group,
    loose_edges,
});
