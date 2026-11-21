use rikka_mesh_modifiers::{BoundaryShape, Solidify, SolidifyMode, ThicknessMode};

use crate::file::Record;

const RIM: i64 = 1;
const EVEN: i64 = 1 << 1;
const HIGH_QUALITY_NORMALS: i64 = 1 << 2;
const INVERT_GROUP: i64 = 1 << 3;
const FLIP: i64 = 1 << 5;
const NO_SHELL: i64 = 1 << 6;
const ANGLE_CLAMP: i64 = 1 << 7;
const FLAT_FACES: i64 = 1 << 8;

pub(super) fn read(modifier: &Record<'_>) -> Solidify {
    let flag = modifier.int("flag").unwrap_or(RIM);
    let defaults = Solidify::default();
    let float = |name: &str, default: f32| modifier.float(name).unwrap_or(default);
    let factor = |name: &str| float(name, 0.0).clamp(0.0, 1.0);
    let short = |name: &str| {
        modifier
            .int(name)
            .unwrap_or(0)
            .clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
    };
    Solidify {
        solidify_mode: match modifier.int("mode").unwrap_or(0) {
            1 => SolidifyMode::Complex,
            _ => SolidifyMode::Simple,
        },
        thickness: float("offset", defaults.thickness),
        offset: float("offset_fac", defaults.offset),
        thickness_clamp: float("offset_clamp", 0.0).clamp(0.0, 100.0),
        use_thickness_angle_clamp: flag & ANGLE_CLAMP != 0,
        thickness_vertex_group: factor("offset_fac_vg"),
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flag & INVERT_GROUP != 0,
        shell_vertex_group: modifier.text("shell_defgrp_name").unwrap_or_default(),
        rim_vertex_group: modifier.text("rim_defgrp_name").unwrap_or_default(),
        use_rim: flag & RIM != 0,
        use_rim_only: flag & NO_SHELL != 0,
        use_even_offset: flag & EVEN != 0,
        use_quality_normals: flag & HIGH_QUALITY_NORMALS != 0,
        use_flip_normals: flag & FLIP != 0,
        use_flat_faces: flag & FLAT_FACES != 0,
        edge_crease_inner: factor("crease_inner"),
        edge_crease_outer: factor("crease_outer"),
        edge_crease_rim: factor("crease_rim"),
        material_offset: short("mat_ofs"),
        material_offset_rim: short("mat_ofs_rim"),
        nonmanifold_thickness_mode: match modifier.int("nonmanifold_offset_mode").unwrap_or(2) {
            0 => ThicknessMode::Fixed,
            1 => ThicknessMode::Even,
            _ => ThicknessMode::Constraints,
        },
        nonmanifold_boundary_mode: match modifier.int("nonmanifold_boundary_mode").unwrap_or(0) {
            1 => BoundaryShape::Round,
            2 => BoundaryShape::Flat,
            _ => BoundaryShape::None,
        },
        nonmanifold_merge_threshold: float("merge_tolerance", defaults.nonmanifold_merge_threshold)
            .clamp(0.0, 1.0),
        bevel_convex: float("bevel_convex", 0.0).clamp(-1.0, 1.0),
    }
}
