use crate::codec::record;
use crate::reference::ObjectRef;

#[derive(Clone, Debug, PartialEq)]
pub struct Armature {
    pub object: Option<ObjectRef>,
    pub use_vertex_groups: bool,
    pub use_bone_envelopes: bool,
    pub use_deform_preserve_volume: bool,
    pub use_multi_modifier: bool,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
}

impl Default for Armature {
    fn default() -> Armature {
        Armature {
            object: None,
            use_vertex_groups: true,
            use_bone_envelopes: false,
            use_deform_preserve_volume: false,
            use_multi_modifier: false,
            vertex_group: String::new(),
            invert_vertex_group: false,
        }
    }
}

record!(Armature {
    object,
    use_vertex_groups,
    use_bone_envelopes,
    use_deform_preserve_volume,
    use_multi_modifier,
    vertex_group,
    invert_vertex_group,
});
