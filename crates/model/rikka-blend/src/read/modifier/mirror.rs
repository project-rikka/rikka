use rikka_mesh_modifiers::Mirror;
use rikka_model_core::glam::Vec2;

use super::References;
use crate::file::Record;

const CLIPPING: i64 = 1;
const MIRROR_U: i64 = 1 << 1;
const MIRROR_V: i64 = 1 << 2;
const AXIS_X: i64 = 1 << 3;
const VERTEX_GROUPS: i64 = 1 << 6;
const NO_MERGE: i64 = 1 << 7;
const BISECT_X: i64 = 1 << 8;
const BISECT_FLIP_X: i64 = 1 << 11;
const MIRROR_UDIM: i64 = 1 << 14;

pub(super) fn read<'a>(modifier: &Record<'a>, refs: &dyn References<'a>) -> Mirror {
    let mut flag = modifier.int("flag").unwrap_or(AXIS_X | VERTEX_GROUPS);
    if flag & (AXIS_X | AXIS_X << 1 | AXIS_X << 2) == 0 {
        flag |= AXIS_X << modifier.int("axis").unwrap_or(0).clamp(0, 2);
    }
    let axes = |first: i64| std::array::from_fn(|axis| flag & (first << axis) != 0);
    let pair = |name: &str| {
        modifier
            .floats::<2>(name)
            .map_or(Vec2::ZERO, Vec2::from_array)
    };
    Mirror {
        use_axis: axes(AXIS_X),
        use_bisect_axis: axes(BISECT_X),
        use_bisect_flip_axis: axes(BISECT_FLIP_X),
        mirror_object: refs.object(modifier.id("mirror_ob")),
        use_clip: flag & CLIPPING != 0,
        use_mirror_merge: flag & NO_MERGE == 0,
        merge_threshold: modifier.float("tolerance").unwrap_or(0.001).max(0.0),
        bisect_threshold: modifier.float("bisect_threshold").unwrap_or(0.001).max(0.0),
        use_mirror_vertex_groups: flag & VERTEX_GROUPS != 0,
        use_mirror_u: flag & MIRROR_U != 0,
        use_mirror_v: flag & MIRROR_V != 0,
        use_mirror_udim: flag & MIRROR_UDIM != 0,
        mirror_offset: pair("uv_offset"),
        offset: pair("uv_offset_copy"),
        use_correct_order_on_merge: modifier.int("use_correct_order_on_merge").unwrap_or(0) != 0,
    }
}
