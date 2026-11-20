use std::f32::consts::PI;

use glam::Vec2;

use crate::codec::{counted, finite2, items, non_negative, record, tagged, within};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BevelAffect {
    Vertices,
    #[default]
    Edges,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BevelOffsetType {
    #[default]
    Offset,
    Width,
    Depth,
    Percent,
    Absolute,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BevelLimitMethod {
    None,
    #[default]
    Angle,
    Weight,
    VertexGroup,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BevelProfileType {
    #[default]
    Superellipse,
    Custom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BevelFaceStrength {
    #[default]
    None,
    New,
    Affected,
    All,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BevelMiter {
    #[default]
    Sharp,
    Patch,
    Arc,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BevelVertexMesh {
    #[default]
    Adjacent,
    Cutoff,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProfileHandle {
    #[default]
    Auto,
    Vector,
    Free,
    Aligned,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProfilePreset {
    #[default]
    Line,
    Supports,
    Cornice,
    Crown,
    Steps,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfilePoint {
    pub location: Vec2,
    pub handle_left_type: ProfileHandle,
    pub handle_right_type: ProfileHandle,
    pub handle_left: Vec2,
    pub handle_right: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomProfile {
    pub points: Vec<ProfilePoint>,
    pub preset: ProfilePreset,
    pub use_clip: bool,
    pub use_sample_straight_edges: bool,
    pub use_sample_even_lengths: bool,
}

impl Default for CustomProfile {
    fn default() -> CustomProfile {
        let point = |location: Vec2| ProfilePoint {
            location,
            handle_left_type: ProfileHandle::Vector,
            handle_right_type: ProfileHandle::Vector,
            handle_left: location,
            handle_right: location,
        };
        CustomProfile {
            points: vec![point(Vec2::new(1.0, 0.0)), point(Vec2::new(0.0, 1.0))],
            preset: ProfilePreset::Line,
            use_clip: true,
            use_sample_straight_edges: false,
            use_sample_even_lengths: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bevel {
    pub width: f32,
    pub segments: u32,
    pub affect: BevelAffect,
    pub offset_type: BevelOffsetType,
    pub limit_method: BevelLimitMethod,
    pub angle_limit: f32,
    pub edge_weight: String,
    pub vertex_weight: String,
    pub vertex_group: String,
    pub invert_vertex_group: bool,
    pub profile_type: BevelProfileType,
    pub profile: f32,
    pub custom_profile: CustomProfile,
    pub use_clamp_overlap: bool,
    pub loop_slide: bool,
    pub mark_seam: bool,
    pub mark_sharp: bool,
    pub material: i16,
    pub harden_normals: bool,
    pub face_strength_mode: BevelFaceStrength,
    pub miter_outer: BevelMiter,
    pub miter_inner: BevelMiter,
    pub spread: f32,
    pub vmesh_method: BevelVertexMesh,
}

impl Default for Bevel {
    fn default() -> Bevel {
        Bevel {
            width: 0.1,
            segments: 1,
            affect: BevelAffect::Edges,
            offset_type: BevelOffsetType::Offset,
            limit_method: BevelLimitMethod::Angle,
            angle_limit: 30.0f32.to_radians(),
            edge_weight: String::from("bevel_weight_edge"),
            vertex_weight: String::from("bevel_weight_vert"),
            vertex_group: String::new(),
            invert_vertex_group: false,
            profile_type: BevelProfileType::Superellipse,
            profile: 0.5,
            custom_profile: CustomProfile::default(),
            use_clamp_overlap: true,
            loop_slide: true,
            mark_seam: false,
            mark_sharp: false,
            material: -1,
            harden_normals: false,
            face_strength_mode: BevelFaceStrength::None,
            miter_outer: BevelMiter::Sharp,
            miter_inner: BevelMiter::Sharp,
            spread: 0.1,
            vmesh_method: BevelVertexMesh::Adjacent,
        }
    }
}

tagged!(BevelAffect, "bevel affect mode" {
    Vertices = 0,
    Edges = 1,
});

tagged!(BevelOffsetType, "bevel width type" {
    Offset = 0,
    Width = 1,
    Depth = 2,
    Percent = 3,
    Absolute = 4,
});

tagged!(BevelLimitMethod, "bevel limit method" {
    None = 0,
    Angle = 1,
    Weight = 2,
    VertexGroup = 3,
});

tagged!(BevelProfileType, "bevel profile type" {
    Superellipse = 0,
    Custom = 1,
});

tagged!(BevelFaceStrength, "bevel face strength mode" {
    None = 0,
    New = 1,
    Affected = 2,
    All = 3,
});

tagged!(BevelMiter, "bevel miter" {
    Sharp = 0,
    Patch = 1,
    Arc = 2,
});

tagged!(BevelVertexMesh, "bevel vertex mesh method" {
    Adjacent = 0,
    Cutoff = 1,
});

tagged!(ProfileHandle, "profile handle type" {
    Auto = 0,
    Vector = 1,
    Free = 2,
    Aligned = 3,
});

tagged!(ProfilePreset, "profile preset" {
    Line = 0,
    Supports = 1,
    Cornice = 2,
    Crown = 3,
    Steps = 4,
});

record!(ProfilePoint {
    location => finite2("profile point is not finite"),
    handle_left_type,
    handle_right_type,
    handle_left => finite2("profile handle is not finite"),
    handle_right => finite2("profile handle is not finite"),
});

record!(CustomProfile {
    points => items(2, i16::MAX as usize, "custom profile needs 2 to 32767 points"),
    preset,
    use_clip,
    use_sample_straight_edges,
    use_sample_even_lengths,
});

record!(Bevel {
    width => non_negative("bevel width is negative"),
    segments => counted(1, 1000, "bevel segments are outside 1..1000"),
    affect,
    offset_type,
    limit_method,
    angle_limit => within(0.0, PI, "bevel angle limit is outside 0..180 degrees"),
    edge_weight,
    vertex_weight,
    vertex_group,
    invert_vertex_group,
    profile_type,
    profile => within(0.0, 1.0, "bevel profile is outside 0..1"),
    custom_profile,
    use_clamp_overlap,
    loop_slide,
    mark_seam,
    mark_sharp,
    material => counted(-1, i16::MAX, "bevel material is below -1"),
    harden_normals,
    face_strength_mode,
    miter_outer,
    miter_inner,
    spread => non_negative("bevel spread is negative"),
    vmesh_method,
});
