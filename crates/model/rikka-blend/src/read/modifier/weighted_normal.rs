use rikka_mesh_modifiers::{WeightedNormal, WeightedNormalMode};

use crate::file::Record;

const KEEP_SHARP: i64 = 1;
const INVERT_GROUP: i64 = 1 << 1;
const FACE_INFLUENCE: i64 = 1 << 2;
const MODES: [WeightedNormalMode; 3] = [
    WeightedNormalMode::FaceArea,
    WeightedNormalMode::CornerAngle,
    WeightedNormalMode::FaceAreaAngle,
];

pub(super) fn read(modifier: &Record<'_>) -> WeightedNormal {
    let flag = modifier.int("flag").unwrap_or(0);
    WeightedNormal {
        mode: usize::try_from(modifier.int("mode").unwrap_or(0))
            .ok()
            .and_then(|index| MODES.get(index).copied())
            .unwrap_or_default(),
        weight: modifier.int("weight").unwrap_or(50).clamp(1, 100) as u8,
        threshold: modifier.float("thresh").unwrap_or(0.01).clamp(0.0, 10.0),
        keep_sharp: flag & KEEP_SHARP != 0,
        use_face_influence: flag & FACE_INFLUENCE != 0,
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flag & INVERT_GROUP != 0,
    }
}
