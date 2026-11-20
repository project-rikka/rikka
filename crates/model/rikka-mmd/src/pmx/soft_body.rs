use rikka_model_core::Conversion;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PmxSoftBodyShape {
    #[default]
    TriMesh,
    Rope,
}

impl PmxSoftBodyShape {
    pub const ALL: [PmxSoftBodyShape; 2] = [PmxSoftBodyShape::TriMesh, PmxSoftBodyShape::Rope];

    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        Self::ALL.get(usize::from(byte)).copied()
    }

    #[must_use]
    pub fn byte(self) -> u8 {
        match self {
            Self::TriMesh => 0,
            Self::Rope => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PmxAeroModel {
    #[default]
    VertexPoint,
    VertexTwoSided,
    VertexOneSided,
    FaceTwoSided,
    FaceOneSided,
}

impl PmxAeroModel {
    pub const ALL: [PmxAeroModel; 5] = [
        PmxAeroModel::VertexPoint,
        PmxAeroModel::VertexTwoSided,
        PmxAeroModel::VertexOneSided,
        PmxAeroModel::FaceTwoSided,
        PmxAeroModel::FaceOneSided,
    ];

    #[must_use]
    pub fn from_code(code: i32) -> Option<Self> {
        usize::try_from(code)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
    }

    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            Self::VertexPoint => 0,
            Self::VertexTwoSided => 1,
            Self::VertexOneSided => 2,
            Self::FaceTwoSided => 3,
            Self::FaceOneSided => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PmxSoftBodyFlags(pub u8);

impl PmxSoftBodyFlags {
    pub const BENDING_LINKS: u8 = 0x01;
    pub const CLUSTERS: u8 = 0x02;
    pub const LINK_CROSSING: u8 = 0x04;

    #[must_use]
    pub fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn set(&mut self, flag: u8, enabled: bool) {
        match enabled {
            true => self.0 |= flag,
            false => self.0 &= !flag,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PmxSoftBodyConfig {
    pub velocity_correction: f32,
    pub damping: f32,
    pub drag: f32,
    pub lift: f32,
    pub pressure: f32,
    pub volume_conservation: f32,
    pub dynamic_friction: f32,
    pub pose_matching: f32,
    pub rigid_contact_hardness: f32,
    pub kinetic_contact_hardness: f32,
    pub soft_contact_hardness: f32,
    pub anchor_hardness: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PmxClusterConfig {
    pub soft_rigid_hardness: f32,
    pub soft_kinetic_hardness: f32,
    pub soft_soft_hardness: f32,
    pub soft_rigid_impulse_split: f32,
    pub soft_kinetic_impulse_split: f32,
    pub soft_soft_impulse_split: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PmxSoftBodyIterations {
    pub velocity: i32,
    pub position: i32,
    pub drift: i32,
    pub cluster: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PmxSoftBodyMaterial {
    pub linear_stiffness: f32,
    pub area_stiffness: f32,
    pub volume_stiffness: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PmxAnchor {
    pub body: usize,
    pub vertex: u32,
    pub near_mode: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PmxSoftBody {
    pub name: String,
    pub english_name: String,
    pub shape: PmxSoftBodyShape,
    pub material: Option<usize>,
    pub group: u8,
    pub collision_mask: u16,
    pub flags: PmxSoftBodyFlags,
    pub bending_distance: i32,
    pub cluster_count: i32,
    pub total_mass: f32,
    pub collision_margin: f32,
    pub aero_model: PmxAeroModel,
    pub config: PmxSoftBodyConfig,
    pub cluster: PmxClusterConfig,
    pub iterations: PmxSoftBodyIterations,
    pub material_stiffness: PmxSoftBodyMaterial,
    pub anchors: Vec<PmxAnchor>,
    pub pins: Vec<u32>,
}

impl PmxSoftBody {
    pub(crate) fn convert(&mut self, conversion: &Conversion) {
        self.collision_margin *= conversion.scale();
    }
}
