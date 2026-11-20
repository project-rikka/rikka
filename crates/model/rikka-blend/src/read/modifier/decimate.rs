use rikka_mesh_modifiers::{Decimate, DecimateType, Delimit, SymmetryAxis};

use crate::file::Record;
use crate::write::Struct;

const INVERT_GROUP: i64 = 1;
const TRIANGULATE: i64 = 1 << 1;
const ALL_BOUNDARY: i64 = 1 << 2;
const SYMMETRY: i64 = 1 << 3;
const TYPES: [DecimateType; 3] = [
    DecimateType::Collapse,
    DecimateType::Unsubdivide,
    DecimateType::Planar,
];
const AXES: [SymmetryAxis; 3] = [SymmetryAxis::X, SymmetryAxis::Y, SymmetryAxis::Z];

pub(super) fn read(modifier: &Record<'_>) -> Decimate {
    let flag = modifier.int("flag").unwrap_or(0);
    let pick = |name: &str| usize::try_from(modifier.int(name).unwrap_or(0)).ok();
    Decimate {
        decimate_type: pick("mode")
            .and_then(|index| TYPES.get(index).copied())
            .unwrap_or_default(),
        ratio: modifier.float("percent").unwrap_or(1.0).clamp(0.0, 1.0),
        iterations: modifier
            .int("iter")
            .unwrap_or(0)
            .clamp(0, i64::from(i16::MAX)) as u16,
        angle_limit: modifier
            .float("angle")
            .unwrap_or(5.0f32.to_radians())
            .clamp(0.0, std::f32::consts::PI),
        delimit: Delimit::from_bits_truncate(modifier.int("delimit").unwrap_or(0) as u8),
        use_dissolve_boundaries: flag & ALL_BOUNDARY != 0,
        use_collapse_triangulate: flag & TRIANGULATE != 0,
        use_symmetry: flag & SYMMETRY != 0,
        symmetry_axis: pick("symmetry_axis")
            .and_then(|index| AXES.get(index).copied())
            .unwrap_or_default(),
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flag & INVERT_GROUP != 0,
        vertex_group_factor: modifier
            .float("defgrp_factor")
            .unwrap_or(1.0)
            .clamp(0.0, 1000.0),
    }
}

pub(super) fn write(decimate: &Decimate, record: &mut Struct<'_>) {
    record
        .set_int(
            "mode",
            TYPES
                .iter()
                .position(|kind| *kind == decimate.decimate_type)
                .unwrap_or(0) as i64,
        )
        .set_float("percent", decimate.ratio)
        .set_int("iter", i64::from(decimate.iterations))
        .set_float("angle", decimate.angle_limit)
        .set_int("delimit", i64::from(decimate.delimit.bits()))
        .set_flag("flag", ALL_BOUNDARY, decimate.use_dissolve_boundaries)
        .set_flag("flag", TRIANGULATE, decimate.use_collapse_triangulate)
        .set_flag("flag", SYMMETRY, decimate.use_symmetry)
        .set_int(
            "symmetry_axis",
            AXES.iter()
                .position(|axis| *axis == decimate.symmetry_axis)
                .unwrap_or(0) as i64,
        )
        .set_text("defgrp_name", &decimate.vertex_group)
        .set_flag("flag", INVERT_GROUP, decimate.invert_vertex_group)
        .set_float("defgrp_factor", decimate.vertex_group_factor);
}
