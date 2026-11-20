use rikka_mesh_modifiers::{BoundaryShape, Solidify, SolidifyMode, ThicknessMode};

use crate::file::Record;
use crate::write::Struct;

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

pub(super) fn write(solidify: &Solidify, record: &mut Struct<'_>) {
    record
        .set_int(
            "mode",
            match solidify.solidify_mode {
                SolidifyMode::Complex => 1,
                SolidifyMode::Simple => 0,
            },
        )
        .set_float("offset", solidify.thickness)
        .set_float("offset_fac", solidify.offset)
        .set_float("offset_clamp", solidify.thickness_clamp)
        .set_flag("flag", ANGLE_CLAMP, solidify.use_thickness_angle_clamp)
        .set_float("offset_fac_vg", solidify.thickness_vertex_group)
        .set_text("defgrp_name", &solidify.vertex_group)
        .set_flag("flag", INVERT_GROUP, solidify.invert_vertex_group)
        .set_text("shell_defgrp_name", &solidify.shell_vertex_group)
        .set_text("rim_defgrp_name", &solidify.rim_vertex_group)
        .set_flag("flag", RIM, solidify.use_rim)
        .set_flag("flag", NO_SHELL, solidify.use_rim_only)
        .set_flag("flag", EVEN, solidify.use_even_offset)
        .set_flag("flag", HIGH_QUALITY_NORMALS, solidify.use_quality_normals)
        .set_flag("flag", FLIP, solidify.use_flip_normals)
        .set_flag("flag", FLAT_FACES, solidify.use_flat_faces)
        .set_float("crease_inner", solidify.edge_crease_inner)
        .set_float("crease_outer", solidify.edge_crease_outer)
        .set_float("crease_rim", solidify.edge_crease_rim)
        .set_int("mat_ofs", i64::from(solidify.material_offset))
        .set_int("mat_ofs_rim", i64::from(solidify.material_offset_rim))
        .set_int(
            "nonmanifold_offset_mode",
            match solidify.nonmanifold_thickness_mode {
                ThicknessMode::Fixed => 0,
                ThicknessMode::Even => 1,
                ThicknessMode::Constraints => 2,
            },
        )
        .set_int(
            "nonmanifold_boundary_mode",
            match solidify.nonmanifold_boundary_mode {
                BoundaryShape::None => 0,
                BoundaryShape::Round => 1,
                BoundaryShape::Flat => 2,
            },
        )
        .set_float("merge_tolerance", solidify.nonmanifold_merge_threshold)
        .set_float("bevel_convex", solidify.bevel_convex);
}
