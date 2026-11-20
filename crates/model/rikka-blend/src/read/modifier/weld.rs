use rikka_mesh_modifiers::{Weld, WeldMode};

use crate::file::Record;
use crate::write::Struct;

const INVERT_GROUP: i64 = 1;
const LOOSE_EDGES: i64 = 1 << 1;
const MODE_ALL: i64 = 0;
const MODE_CONNECTED: i64 = 1;

pub(super) fn read(modifier: &Record<'_>) -> Weld {
    let flag = modifier.int("flag").unwrap_or(0);
    Weld {
        mode: match modifier.int("mode").unwrap_or(MODE_ALL) {
            MODE_CONNECTED => WeldMode::Connected,
            _ => WeldMode::All,
        },
        merge_threshold: modifier.float("merge_dist").unwrap_or(0.001).max(0.0),
        vertex_group: modifier.text("defgrp_name").unwrap_or_default(),
        invert_vertex_group: flag & INVERT_GROUP != 0,
        loose_edges: flag & LOOSE_EDGES != 0,
    }
}

pub(super) fn write(weld: &Weld, record: &mut Struct<'_>) {
    record
        .set_int(
            "mode",
            match weld.mode {
                WeldMode::All => MODE_ALL,
                WeldMode::Connected => MODE_CONNECTED,
            },
        )
        .set_float("merge_dist", weld.merge_threshold)
        .set_text("defgrp_name", &weld.vertex_group)
        .set_flag("flag", INVERT_GROUP, weld.invert_vertex_group)
        .set_flag("flag", LOOSE_EDGES, weld.loose_edges);
}
