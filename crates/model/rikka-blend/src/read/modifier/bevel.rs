use rikka_mesh_modifiers::{
    Bevel, BevelAffect, BevelFaceStrength, BevelLimitMethod, BevelMiter, BevelOffsetType,
    BevelProfileType, BevelVertexMesh, CustomProfile, ProfileHandle, ProfilePoint, ProfilePreset,
};
use rikka_model_core::glam::Vec2;

use crate::file::Record;

const INVERT_GROUP: i64 = 1 << 2;
const LIMIT_ANGLE: i64 = 1 << 3;
const LIMIT_WEIGHT: i64 = 1 << 4;
const LIMIT_GROUP: i64 = 1 << 5;
const OVERLAP_OK: i64 = 1 << 13;
const EVEN_WIDTHS: i64 = 1 << 14;
const HARDEN_NORMALS: i64 = 1 << 15;
const MARK_SEAM: i64 = 1;
const MARK_SHARP: i64 = 1 << 1;
const PROFILE_CLIP: i64 = 1;
const PROFILE_STRAIGHT_EDGES: i64 = 1 << 2;
const PROFILE_EVEN_LENGTHS: i64 = 1 << 3;
const OFFSET_TYPES: [BevelOffsetType; 5] = [
    BevelOffsetType::Offset,
    BevelOffsetType::Width,
    BevelOffsetType::Depth,
    BevelOffsetType::Percent,
    BevelOffsetType::Absolute,
];
const FACE_STRENGTHS: [BevelFaceStrength; 4] = [
    BevelFaceStrength::None,
    BevelFaceStrength::New,
    BevelFaceStrength::Affected,
    BevelFaceStrength::All,
];
const MITERS: [BevelMiter; 3] = [BevelMiter::Sharp, BevelMiter::Patch, BevelMiter::Arc];
const PRESETS: [ProfilePreset; 5] = [
    ProfilePreset::Line,
    ProfilePreset::Supports,
    ProfilePreset::Cornice,
    ProfilePreset::Crown,
    ProfilePreset::Steps,
];
const HANDLES: [ProfileHandle; 4] = [
    ProfileHandle::Free,
    ProfileHandle::Auto,
    ProfileHandle::Vector,
    ProfileHandle::Aligned,
];

fn pick<T: Copy>(table: &[T], value: Option<i64>, default: T) -> T {
    value
        .and_then(|value| usize::try_from(value).ok())
        .and_then(|index| table.get(index).copied())
        .unwrap_or(default)
}

pub(super) fn read(modifier: &Record<'_>) -> Bevel {
    let defaults = Bevel::default();
    let flags = modifier.int("flags").unwrap_or(0);
    let limits = modifier.int("lim_flags").unwrap_or(LIMIT_ANGLE);
    let edge_flags = modifier.int("edge_flags").unwrap_or(0);
    Bevel {
        width: modifier.float("value").unwrap_or(defaults.width).max(0.0),
        segments: modifier.int("res").unwrap_or(1).clamp(1, 1000) as u32,
        affect: match modifier.int("affect_type").unwrap_or(1) {
            0 => BevelAffect::Vertices,
            _ => BevelAffect::Edges,
        },
        offset_type: pick(
            &OFFSET_TYPES,
            modifier.int("val_flags"),
            BevelOffsetType::Offset,
        ),
        limit_method: match () {
            () if limits & LIMIT_ANGLE != 0 => BevelLimitMethod::Angle,
            () if limits & LIMIT_WEIGHT != 0 => BevelLimitMethod::Weight,
            () if limits & LIMIT_GROUP != 0 => BevelLimitMethod::VertexGroup,
            () => BevelLimitMethod::None,
        },
        angle_limit: modifier
            .float("bevel_angle")
            .unwrap_or(defaults.angle_limit)
            .clamp(0.0, std::f32::consts::PI),
        edge_weight: modifier
            .text("edge_weight_name")
            .unwrap_or(defaults.edge_weight),
        vertex_weight: modifier
            .text("vertex_weight_name")
            .unwrap_or(defaults.vertex_weight),
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flags & INVERT_GROUP != 0,
        profile_type: match modifier.int("profile_type").unwrap_or(0) {
            1 => BevelProfileType::Custom,
            _ => BevelProfileType::Superellipse,
        },
        profile: modifier.float("profile").unwrap_or(0.5).clamp(0.0, 1.0),
        custom_profile: modifier
            .deref("custom_profile")
            .and_then(|profile| read_profile(&profile))
            .unwrap_or_default(),
        use_clamp_overlap: flags & OVERLAP_OK == 0,
        loop_slide: flags & EVEN_WIDTHS == 0,
        mark_seam: edge_flags & MARK_SEAM != 0,
        mark_sharp: edge_flags & MARK_SHARP != 0,
        material: modifier
            .int("mat")
            .unwrap_or(-1)
            .clamp(-1, i64::from(i16::MAX)) as i16,
        harden_normals: flags & HARDEN_NORMALS != 0,
        face_strength_mode: pick(
            &FACE_STRENGTHS,
            modifier.int("face_str_mode"),
            BevelFaceStrength::None,
        ),
        miter_outer: pick(&MITERS, modifier.int("miter_outer"), BevelMiter::Sharp),
        miter_inner: pick(&MITERS, modifier.int("miter_inner"), BevelMiter::Sharp),
        spread: modifier.float("spread").unwrap_or(defaults.spread).max(0.0),
        vmesh_method: match modifier.int("vmesh_method").unwrap_or(0) {
            1 => BevelVertexMesh::Cutoff,
            _ => BevelVertexMesh::Adjacent,
        },
    }
}

fn read_profile(profile: &Record<'_>) -> Option<CustomProfile> {
    let count = profile.int("path_len").unwrap_or(0).max(0) as usize;
    let points: Vec<ProfilePoint> = profile
        .table("path", "CurveProfilePoint")?
        .rows()
        .take(count.min(i16::MAX as usize))
        .map(|point| {
            let pair = |name: &str| point.floats::<2>(name).map_or(Vec2::ZERO, Vec2::from_array);
            ProfilePoint {
                location: Vec2::new(
                    point.float("x").unwrap_or(0.0),
                    point.float("y").unwrap_or(0.0),
                ),
                handle_left_type: pick(&HANDLES, point.int("h1"), ProfileHandle::Auto),
                handle_right_type: pick(&HANDLES, point.int("h2"), ProfileHandle::Auto),
                handle_left: pair("h1_loc"),
                handle_right: pair("h2_loc"),
            }
        })
        .collect();
    if points.len() < 2 {
        return None;
    }
    let flag = profile.int("flag").unwrap_or(PROFILE_CLIP);
    Some(CustomProfile {
        points,
        preset: pick(&PRESETS, profile.int("preset"), ProfilePreset::Line),
        use_clip: flag & PROFILE_CLIP != 0,
        use_sample_straight_edges: flag & PROFILE_STRAIGHT_EDGES != 0,
        use_sample_even_lengths: flag & PROFILE_EVEN_LENGTHS != 0,
    })
}
