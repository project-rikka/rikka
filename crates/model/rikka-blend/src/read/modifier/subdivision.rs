use rikka_mesh_modifiers::{AdaptiveSpace, BoundarySmooth, Subdivision, SubdivisionType, UvSmooth};

use crate::file::Record;

const SIMPLE: i64 = 1;
const CONTROL_EDGES: i64 = 1 << 2;
const USE_CREASE: i64 = 1 << 4;
const USE_CUSTOM_NORMALS: i64 = 1 << 5;
const RECURSIVE: i64 = 1 << 6;
const ADAPTIVE: i64 = 1 << 7;
const UV_SMOOTH: [UvSmooth; 6] = [
    UvSmooth::None,
    UvSmooth::PreserveCorners,
    UvSmooth::PreserveCornersAndJunctions,
    UvSmooth::PreserveCornersJunctionsAndConcave,
    UvSmooth::PreserveBoundaries,
    UvSmooth::All,
];

pub(super) fn read(modifier: &Record<'_>) -> Subdivision {
    let flags = modifier.int("flags").unwrap_or(USE_CREASE);
    let levels = |name: &str, default: i64| {
        modifier
            .int(name)
            .unwrap_or(default)
            .clamp(0, i64::from(Subdivision::MAX_LEVELS)) as u8
    };
    let defaults = Subdivision::default();
    Subdivision {
        subdivision_type: match modifier.int("subdivType").unwrap_or(0) {
            SIMPLE => SubdivisionType::Simple,
            _ => SubdivisionType::CatmullClark,
        },
        levels: levels("levels", 1),
        render_levels: levels("renderLevels", 2),
        quality: modifier.int("quality").unwrap_or(3).clamp(1, 10) as u8,
        show_only_control_edges: flags & CONTROL_EDGES != 0,
        use_creases: flags & USE_CREASE != 0,
        use_custom_normals: flags & USE_CUSTOM_NORMALS != 0,
        use_limit_surface: flags & RECURSIVE == 0,
        uv_smooth: usize::try_from(modifier.int("uv_smooth").unwrap_or(4))
            .ok()
            .and_then(|index| UV_SMOOTH.get(index).copied())
            .unwrap_or(UvSmooth::PreserveBoundaries),
        boundary_smooth: match modifier.int("boundary_smooth").unwrap_or(0) {
            1 => BoundarySmooth::PreserveCorners,
            _ => BoundarySmooth::All,
        },
        use_adaptive_subdivision: flags & ADAPTIVE != 0,
        adaptive_space: match modifier.int("adaptive_space").unwrap_or(0) {
            1 => AdaptiveSpace::Object,
            _ => AdaptiveSpace::Pixel,
        },
        adaptive_pixel_size: modifier
            .float("adaptive_pixel_size")
            .unwrap_or(defaults.adaptive_pixel_size)
            .clamp(0.1, 1000.0),
        adaptive_object_edge_length: modifier
            .float("adaptive_object_edge_length")
            .unwrap_or(defaults.adaptive_object_edge_length)
            .clamp(0.0001, 1000.0),
    }
}
