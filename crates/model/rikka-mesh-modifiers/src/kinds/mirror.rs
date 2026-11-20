use glam::Vec2;

use crate::codec::{non_negative, record, within2};
use crate::reference::ObjectRef;

#[derive(Clone, Debug, PartialEq)]
pub struct Mirror {
    pub use_axis: [bool; 3],
    pub use_bisect_axis: [bool; 3],
    pub use_bisect_flip_axis: [bool; 3],
    pub mirror_object: Option<ObjectRef>,
    pub use_clip: bool,
    pub use_mirror_merge: bool,
    pub merge_threshold: f32,
    pub bisect_threshold: f32,
    pub use_mirror_vertex_groups: bool,
    pub use_mirror_u: bool,
    pub use_mirror_v: bool,
    pub use_mirror_udim: bool,
    pub mirror_offset: Vec2,
    pub offset: Vec2,
    pub use_correct_order_on_merge: bool,
}

impl Default for Mirror {
    fn default() -> Mirror {
        Mirror {
            use_axis: [true, false, false],
            use_bisect_axis: [false; 3],
            use_bisect_flip_axis: [false; 3],
            mirror_object: None,
            use_clip: false,
            use_mirror_merge: true,
            merge_threshold: 0.001,
            bisect_threshold: 0.001,
            use_mirror_vertex_groups: true,
            use_mirror_u: false,
            use_mirror_v: false,
            use_mirror_udim: false,
            mirror_offset: Vec2::ZERO,
            offset: Vec2::ZERO,
            use_correct_order_on_merge: true,
        }
    }
}

record!(Mirror {
    use_axis,
    use_bisect_axis,
    use_bisect_flip_axis,
    mirror_object,
    use_clip,
    use_mirror_merge,
    merge_threshold => non_negative("mirror merge threshold is negative"),
    bisect_threshold => non_negative("mirror bisect threshold is negative"),
    use_mirror_vertex_groups,
    use_mirror_u,
    use_mirror_v,
    use_mirror_udim,
    mirror_offset => within2(-1.0, 1.0, "mirror UV offset is outside -1..1"),
    offset => within2(-10000.0, 10000.0, "mirror UV copy offset is out of range"),
    use_correct_order_on_merge,
});
