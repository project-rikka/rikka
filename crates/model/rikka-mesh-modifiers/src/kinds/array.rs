use glam::{Vec2, Vec3};

use crate::codec::{counted, finite3, non_negative, record, tagged, within2};
use crate::reference::ObjectRef;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FitType {
    #[default]
    FixedCount,
    FitLength,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    pub fit_type: FitType,
    pub count: u32,
    pub fit_length: f32,
    pub use_constant_offset: bool,
    pub constant_offset_displace: Vec3,
    pub use_relative_offset: bool,
    pub relative_offset_displace: Vec3,
    pub use_object_offset: bool,
    pub offset_object: Option<ObjectRef>,
    pub use_merge_vertices: bool,
    pub use_merge_vertices_cap: bool,
    pub merge_threshold: f32,
    pub start_cap: Option<ObjectRef>,
    pub end_cap: Option<ObjectRef>,
    pub offset_uv: Vec2,
}

impl Default for Array {
    fn default() -> Array {
        Array {
            fit_type: FitType::FixedCount,
            count: 2,
            fit_length: 0.0,
            use_constant_offset: false,
            constant_offset_displace: Vec3::X,
            use_relative_offset: true,
            relative_offset_displace: Vec3::X,
            use_object_offset: false,
            offset_object: None,
            use_merge_vertices: false,
            use_merge_vertices_cap: false,
            merge_threshold: 0.01,
            start_cap: None,
            end_cap: None,
            offset_uv: Vec2::ZERO,
        }
    }
}

tagged!(FitType, "array fit type" {
    FixedCount = 0,
    FitLength = 1,
});

record!(Array {
    fit_type,
    count => counted(1, i32::MAX as u32, "array count is outside 1..2^31"),
    fit_length => non_negative("array fit length is negative"),
    use_constant_offset,
    constant_offset_displace => finite3("array constant offset is not finite"),
    use_relative_offset,
    relative_offset_displace => finite3("array relative offset is not finite"),
    use_object_offset,
    offset_object,
    use_merge_vertices,
    use_merge_vertices_cap,
    merge_threshold => non_negative("array merge threshold is negative"),
    start_cap,
    end_cap,
    offset_uv => within2(-1.0, 1.0, "array UV offset is outside -1..1"),
});
