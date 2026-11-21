use rikka_mesh_modifiers::Armature;

use super::References;
use crate::file::Record;

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
