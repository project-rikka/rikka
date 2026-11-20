use crate::codec::{counted, finite, non_negative, record, tagged};
use crate::reference::ObjectRef;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WrapMethod {
    #[default]
    NearestSurfacePoint,
    Project,
    NearestVertex,
    TargetProject,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WrapMode {
    #[default]
    OnSurface,
    Inside,
    Outside,
    OutsideSurface,
    AboveSurface,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CullFace {
    #[default]
    Off,
    Front,
    Back,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shrinkwrap {
    pub wrap_method: WrapMethod,
    pub wrap_mode: WrapMode,
    pub target: Option<ObjectRef>,
    pub auxiliary_target: Option<ObjectRef>,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
    pub offset: f32,
    pub project_limit: f32,
    pub use_project: [bool; 3],
    pub subsurf_levels: u8,
    pub use_negative_direction: bool,
    pub use_positive_direction: bool,
    pub cull_face: CullFace,
    pub use_invert_cull: bool,
}

impl Default for Shrinkwrap {
    fn default() -> Shrinkwrap {
        Shrinkwrap {
            wrap_method: WrapMethod::NearestSurfacePoint,
            wrap_mode: WrapMode::OnSurface,
            target: None,
            auxiliary_target: None,
            vertex_group: String::new(),
            invert_vertex_group: false,
            offset: 0.0,
            project_limit: 0.0,
            use_project: [false; 3],
            subsurf_levels: 0,
            use_negative_direction: false,
            use_positive_direction: true,
            cull_face: CullFace::Off,
            use_invert_cull: false,
        }
    }
}

tagged!(WrapMethod, "shrinkwrap method" {
    NearestSurfacePoint = 0,
    Project = 1,
    NearestVertex = 2,
    TargetProject = 3,
});

tagged!(WrapMode, "shrinkwrap snap mode" {
    OnSurface = 0,
    Inside = 1,
    Outside = 2,
    OutsideSurface = 3,
    AboveSurface = 4,
});

tagged!(CullFace, "shrinkwrap face culling" {
    Off = 0,
    Front = 1,
    Back = 2,
});

record!(Shrinkwrap {
    wrap_method,
    wrap_mode,
    target,
    auxiliary_target,
    vertex_group,
    invert_vertex_group,
    offset => finite("shrinkwrap offset is not finite"),
    project_limit => non_negative("shrinkwrap projection limit is negative"),
    use_project,
    subsurf_levels => counted(0, 6, "shrinkwrap subdivision levels exceed 6"),
    use_negative_direction,
    use_positive_direction,
    cull_face,
    use_invert_cull,
});
