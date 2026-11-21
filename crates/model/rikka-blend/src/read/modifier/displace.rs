use rikka_mesh_modifiers::{Displace, DisplaceDirection, DisplaceSpace, TextureCoordinates};

use super::References;
use crate::file::Record;

const INVERT_GROUP: i64 = 1;
const COORDINATES: [TextureCoordinates; 4] = [
    TextureCoordinates::Local,
    TextureCoordinates::Global,
    TextureCoordinates::Object,
    TextureCoordinates::Uv,
];
const DIRECTIONS: [DisplaceDirection; 6] = [
    DisplaceDirection::X,
    DisplaceDirection::Y,
    DisplaceDirection::Z,
    DisplaceDirection::Normal,
    DisplaceDirection::RgbToXyz,
    DisplaceDirection::CustomNormal,
];

pub(super) fn read<'a>(modifier: &Record<'a>, refs: &dyn References<'a>) -> Displace {
    let pick =
        |name: &str, default: i64| usize::try_from(modifier.int(name).unwrap_or(default)).ok();
    Displace {
        texture: refs.texture(modifier.id("texture")),
        texture_coords: pick("texmapping", 0)
            .and_then(|index| COORDINATES.get(index).copied())
            .unwrap_or_default(),
        texture_coords_object: refs.object(modifier.id("map_object")),
        texture_coords_bone: modifier.text("map_bone").unwrap_or_default(),
        uv_layer: modifier.text("uvlayer_name").unwrap_or_default(),
        direction: pick("direction", 3)
            .and_then(|index| DIRECTIONS.get(index).copied())
            .unwrap_or_default(),
        space: match modifier.int("space").unwrap_or(0) {
            1 => DisplaceSpace::Global,
            _ => DisplaceSpace::Local,
        },
        strength: modifier.float("strength").unwrap_or(1.0),
        mid_level: modifier.float("midlevel").unwrap_or(0.5),
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: modifier.int("flag").unwrap_or(0) & INVERT_GROUP != 0,
    }
}
