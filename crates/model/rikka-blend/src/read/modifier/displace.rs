use rikka_mesh_modifiers::{Displace, DisplaceDirection, DisplaceSpace, TextureCoordinates};

use super::{References, Targets};
use crate::file::Record;
use crate::write::Struct;

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

pub(super) fn write(displace: &Displace, record: &mut Struct<'_>, targets: &dyn Targets) {
    let index = |position: Option<usize>| position.unwrap_or(0) as i64;
    record
        .set_pointer("texture", targets.texture(displace.texture))
        .set_int(
            "texmapping",
            index(
                COORDINATES
                    .iter()
                    .position(|mode| *mode == displace.texture_coords),
            ),
        )
        .set_pointer("map_object", targets.object(displace.texture_coords_object))
        .set_text("map_bone", &displace.texture_coords_bone)
        .set_text("uvlayer_name", &displace.uv_layer)
        .set_int(
            "direction",
            index(
                DIRECTIONS
                    .iter()
                    .position(|mode| *mode == displace.direction),
            ),
        )
        .set_int(
            "space",
            match displace.space {
                DisplaceSpace::Local => 0,
                DisplaceSpace::Global => 1,
            },
        )
        .set_float("strength", displace.strength)
        .set_float("midlevel", displace.mid_level)
        .set_text("defgrp_name", &displace.vertex_group)
        .set_flag("flag", INVERT_GROUP, displace.invert_vertex_group);
}
