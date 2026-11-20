use std::f32::consts::{FRAC_PI_6, PI};

use rikka_mesh_modifiers::EdgeSplit;

use crate::file::Record;
use crate::write::Struct;

const FROM_ANGLE: i64 = 1 << 1;
const FROM_FLAG: i64 = 1 << 2;

pub(super) fn read(modifier: &Record<'_>) -> EdgeSplit {
    let flags = modifier.int("flags").unwrap_or(FROM_ANGLE | FROM_FLAG);
    EdgeSplit {
        split_angle: modifier
            .float("split_angle")
            .unwrap_or(FRAC_PI_6)
            .clamp(0.0, PI),
        use_edge_angle: flags & FROM_ANGLE != 0,
        use_edge_sharp: flags & FROM_FLAG != 0,
    }
}

pub(super) fn write(split: &EdgeSplit, record: &mut Struct<'_>) {
    record
        .set_float("split_angle", split.split_angle)
        .set_flag("flags", FROM_ANGLE, split.use_edge_angle)
        .set_flag("flags", FROM_FLAG, split.use_edge_sharp);
}
