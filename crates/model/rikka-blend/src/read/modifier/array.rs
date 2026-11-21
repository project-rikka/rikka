use rikka_mesh_modifiers::{Array, FitType};
use rikka_model_core::glam::{Vec2, Vec3};

use super::References;
use crate::file::Record;

const FIT_COUNT: i64 = 0;
const FIT_LENGTH: i64 = 1;
const FIT_CURVE: i64 = 2;
const OFFSET_CONSTANT: i64 = 1;
const OFFSET_RELATIVE: i64 = 1 << 1;
const OFFSET_OBJECT: i64 = 1 << 2;
const MERGE: i64 = 1;
const MERGE_FINAL: i64 = 1 << 1;

pub(super) struct Parsed {
    pub(super) array: Array,
    pub(super) fit_curve: bool,
}

pub(super) fn read<'a>(modifier: &Record<'a>, refs: &dyn References<'a>) -> Parsed {
    let fit = modifier.int("fit_type").unwrap_or(FIT_COUNT);
    let offset_type = modifier.int("offset_type").unwrap_or(OFFSET_RELATIVE);
    let flags = modifier.int("flags").unwrap_or(0);
    Parsed {
        array: Array {
            fit_type: match fit {
                FIT_LENGTH | FIT_CURVE => FitType::FitLength,
                _ => FitType::FixedCount,
            },
            count: modifier
                .int("count")
                .unwrap_or(2)
                .clamp(1, i64::from(i32::MAX)) as u32,
            fit_length: modifier.float("length").unwrap_or(0.0).max(0.0),
            use_constant_offset: offset_type & OFFSET_CONSTANT != 0,
            constant_offset_displace: modifier.vec3("offset").unwrap_or(Vec3::X),
            use_relative_offset: offset_type & OFFSET_RELATIVE != 0,
            relative_offset_displace: modifier.vec3("scale").unwrap_or(Vec3::X),
            use_object_offset: offset_type & OFFSET_OBJECT != 0,
            offset_object: refs.object(modifier.id("offset_ob")),
            use_merge_vertices: flags & MERGE != 0,
            use_merge_vertices_cap: flags & MERGE_FINAL != 0,
            merge_threshold: modifier.float("merge_dist").unwrap_or(0.01).max(0.0),
            start_cap: refs.object(modifier.id("start_cap")),
            end_cap: refs.object(modifier.id("end_cap")),
            offset_uv: modifier
                .floats::<2>("uv_offset")
                .map_or(Vec2::ZERO, Vec2::from_array),
        },
        fit_curve: fit == FIT_CURVE,
    }
}
