use std::f32::consts::{FRAC_PI_6, PI};

use rikka_mesh_modifiers::EdgeSplit;

use crate::file::Record;

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
