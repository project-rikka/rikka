use rikka_mesh_modifiers::{CorrectiveSmooth, RestSource, SmoothType};
use rikka_model_core::glam::Vec3;

use crate::file::{Record, f32s};

const INVERT_GROUP: i64 = 1;
const ONLY_SMOOTH: i64 = 1 << 1;
const PIN_BOUNDARY: i64 = 1 << 2;

pub(super) fn read(modifier: &Record<'_>) -> CorrectiveSmooth {
    let flag = modifier.int("flag").unwrap_or(0);
    let count = usize::try_from(modifier.int("bind_coords_num").unwrap_or(0)).unwrap_or(0);
    let bind_coords: Vec<Vec3> = match modifier.raw("bind_coords") {
        Some(bytes) if count > 0 => f32s(bytes, modifier.blend().endian())
            .as_chunks::<3>()
            .0
            .iter()
            .take(count)
            .map(|&xyz| Vec3::from_array(xyz))
            .filter(|point| point.is_finite())
            .collect(),
        _ => Vec::new(),
    };
    CorrectiveSmooth {
        factor: finite(modifier.float("lambda"), 0.5),
        iterations: modifier
            .int("repeat")
            .unwrap_or(5)
            .clamp(0, i64::from(i16::MAX)) as u16,
        scale: finite(modifier.float("scale"), 1.0),
        smooth_type: match modifier.int("smooth_type") {
            Some(1) => SmoothType::LengthWeighted,
            _ => SmoothType::Simple,
        },
        rest_source: match modifier.int("rest_source") {
            Some(1) => RestSource::Bind,
            _ => RestSource::Original,
        },
        bind_coords: match bind_coords.len() == count {
            true => bind_coords,
            false => Vec::new(),
        },
        use_only_smooth: flag & ONLY_SMOOTH != 0,
        use_pin_boundary: flag & PIN_BOUNDARY != 0,
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flag & INVERT_GROUP != 0,
    }
}

fn finite(value: Option<f32>, default: f32) -> f32 {
    value.filter(|value| value.is_finite()).unwrap_or(default)
}
