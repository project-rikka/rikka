use rikka_mesh_modifiers::{CullFace, Shrinkwrap, WrapMethod, WrapMode};

use super::References;
use crate::file::Record;

const POSITIVE: i64 = 1;
const NEGATIVE: i64 = 1 << 1;
const CULL_FRONT: i64 = 1 << 3;
const CULL_BACK: i64 = 1 << 4;
const INVERT_GROUP: i64 = 1 << 6;
const INVERT_CULL: i64 = 1 << 7;
const METHODS: [WrapMethod; 4] = [
    WrapMethod::NearestSurfacePoint,
    WrapMethod::Project,
    WrapMethod::NearestVertex,
    WrapMethod::TargetProject,
];
const MODES: [WrapMode; 5] = [
    WrapMode::OnSurface,
    WrapMode::Inside,
    WrapMode::Outside,
    WrapMode::OutsideSurface,
    WrapMode::AboveSurface,
];

pub(super) fn read<'a>(modifier: &Record<'a>, refs: &dyn References<'a>) -> Shrinkwrap {
    let options = modifier.int("shrinkOpts").unwrap_or(POSITIVE);
    let axis = modifier.int("projAxis").unwrap_or(0);
    let pick = |name: &str| usize::try_from(modifier.int(name).unwrap_or(0)).ok();
    Shrinkwrap {
        wrap_method: pick("shrinkType")
            .and_then(|index| METHODS.get(index).copied())
            .unwrap_or_default(),
        wrap_mode: pick("shrinkMode")
            .and_then(|index| MODES.get(index).copied())
            .unwrap_or_default(),
        target: refs.object(modifier.id("target")),
        auxiliary_target: refs.object(modifier.id("auxTarget")),
        vertex_group: modifier.text("vgroup_name").unwrap_or_default(),
        invert_vertex_group: options & INVERT_GROUP != 0,
        offset: modifier.float("keepDist").unwrap_or(0.0),
        project_limit: modifier.float("projLimit").unwrap_or(0.0).max(0.0),
        use_project: std::array::from_fn(|index| axis & (1 << index) != 0),
        subsurf_levels: modifier.int("subsurfLevels").unwrap_or(0).clamp(0, 6) as u8,
        use_negative_direction: options & NEGATIVE != 0,
        use_positive_direction: options & POSITIVE != 0,
        cull_face: match (options & CULL_FRONT != 0, options & CULL_BACK != 0) {
            (true, _) => CullFace::Front,
            (false, true) => CullFace::Back,
            (false, false) => CullFace::Off,
        },
        use_invert_cull: options & INVERT_CULL != 0,
    }
}
