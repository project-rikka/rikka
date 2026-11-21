use rikka_mesh_modifiers::Triangulate;
use rikka_polymesh::{NgonMethod, QuadMethod};

use crate::file::Record;

const QUAD_FIXED: i64 = 1;
const QUAD_ALTERNATE: i64 = 2;
const QUAD_SHORT_EDGE: i64 = 3;
const QUAD_LONG_EDGE: i64 = 4;
const NGON_BEAUTY: i64 = 0;
const NGON_CLIP: i64 = 1;
const KEEP_CUSTOM_NORMALS: i64 = 1 << 1;

pub(super) fn read(modifier: &Record<'_>) -> Triangulate {
    Triangulate {
        quad_method: match modifier.int("quad_method").unwrap_or(QUAD_SHORT_EDGE) {
            QUAD_FIXED => QuadMethod::Fixed,
            QUAD_ALTERNATE => QuadMethod::Alternate,
            QUAD_SHORT_EDGE => QuadMethod::ShortEdge,
            QUAD_LONG_EDGE => QuadMethod::LongEdge,
            _ => QuadMethod::Beauty,
        },
        ngon_method: match modifier.int("ngon_method").unwrap_or(NGON_BEAUTY) {
            NGON_CLIP => NgonMethod::Clip,
            _ => NgonMethod::Beauty,
        },
        min_vertices: modifier
            .int("min_vertices")
            .unwrap_or(4)
            .clamp(4, i64::from(i32::MAX)) as u32,
        keep_custom_normals: modifier.int("flag").unwrap_or(0) & KEEP_CUSTOM_NORMALS != 0,
    }
}
