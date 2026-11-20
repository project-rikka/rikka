use rikka_model::glam::Mat4;
use rikka_renderer_3d::EmitterShape;
use rikka_shader_graph::NodeId;

#[derive(Clone, Debug, PartialEq)]
pub struct EmitterMark {
    pub node: Vec<NodeId>,
    pub shape: EmitterShape,
    pub transform: Mat4,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmitterMarks {
    pub view_projection: Mat4,
    pub marks: Vec<EmitterMark>,
}

impl EmitterMarks {
    #[must_use]
    pub fn of(&self, node: &[NodeId]) -> Option<&EmitterMark> {
        self.marks.iter().find(|mark| mark.node == node)
    }
}
