use rikka_mesh_modifiers::{CullFace, Shrinkwrap, WrapMethod, WrapMode};

use super::{References, Targets};
use crate::file::Record;
use crate::write::Struct;

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

pub(super) fn write(shrinkwrap: &Shrinkwrap, record: &mut Struct<'_>, targets: &dyn Targets) {
    let axes = shrinkwrap
        .use_project
        .iter()
        .enumerate()
        .filter(|(_, used)| **used)
        .fold(0, |bits, (axis, _)| bits | 1 << axis);
    record
        .set_int(
            "shrinkType",
            METHODS
                .iter()
                .position(|method| *method == shrinkwrap.wrap_method)
                .unwrap_or(0) as i64,
        )
        .set_int(
            "shrinkMode",
            MODES
                .iter()
                .position(|mode| *mode == shrinkwrap.wrap_mode)
                .unwrap_or(0) as i64,
        )
        .set_pointer("target", targets.object(shrinkwrap.target))
        .set_pointer("auxTarget", targets.object(shrinkwrap.auxiliary_target))
        .set_text("vgroup_name", &shrinkwrap.vertex_group)
        .set_flag("shrinkOpts", INVERT_GROUP, shrinkwrap.invert_vertex_group)
        .set_float("keepDist", shrinkwrap.offset)
        .set_float("projLimit", shrinkwrap.project_limit)
        .set_int("projAxis", axes)
        .set_int("subsurfLevels", i64::from(shrinkwrap.subsurf_levels))
        .set_flag("shrinkOpts", NEGATIVE, shrinkwrap.use_negative_direction)
        .set_flag("shrinkOpts", POSITIVE, shrinkwrap.use_positive_direction)
        .set_flag(
            "shrinkOpts",
            CULL_FRONT,
            shrinkwrap.cull_face == CullFace::Front,
        )
        .set_flag(
            "shrinkOpts",
            CULL_BACK,
            shrinkwrap.cull_face == CullFace::Back,
        )
        .set_flag("shrinkOpts", INVERT_CULL, shrinkwrap.use_invert_cull);
}
