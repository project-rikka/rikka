use rikka_mesh_modifiers::Armature;

use super::{References, Targets};
use crate::file::Record;
use crate::write::Struct;

const VERTEX_GROUPS: i64 = 1;
const ENVELOPES: i64 = 1 << 1;
const PRESERVE_VOLUME: i64 = 1 << 2;
const INVERT_GROUP: i64 = 1 << 4;

pub(super) fn read<'a>(modifier: &Record<'a>, refs: &dyn References<'a>) -> Armature {
    let flags = modifier.int("deformflag").unwrap_or(VERTEX_GROUPS);
    Armature {
        object: refs.object(modifier.id("object")),
        use_vertex_groups: flags & VERTEX_GROUPS != 0,
        use_bone_envelopes: flags & ENVELOPES != 0,
        use_deform_preserve_volume: flags & PRESERVE_VOLUME != 0,
        use_multi_modifier: modifier.int("multi").unwrap_or(0) != 0,
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flags & INVERT_GROUP != 0,
    }
}

pub(super) fn write(armature: &Armature, record: &mut Struct<'_>, targets: &dyn Targets) {
    record
        .set_pointer("object", targets.object(armature.object))
        .set_flag("deformflag", VERTEX_GROUPS, armature.use_vertex_groups)
        .set_flag("deformflag", ENVELOPES, armature.use_bone_envelopes)
        .set_flag(
            "deformflag",
            PRESERVE_VOLUME,
            armature.use_deform_preserve_volume,
        )
        .set_flag("deformflag", INVERT_GROUP, armature.invert_vertex_group)
        .set_bool("multi", armature.use_multi_modifier)
        .set_text("defgrp_name", &armature.vertex_group);
}
