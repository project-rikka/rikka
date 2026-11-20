use rikka_model_core::glam::{Quat, Vec3, Vec4};

use rikka_model_core::Conversion;
use rikka_model_core::Extension;
use rikka_model_core::MaterialAlias;
use rikka_model_core::MaterialId;
use rikka_model_core::MeshId;
use rikka_model_core::Model;
use rikka_model_core::NodeId;
use rikka_model_core::Rig;
use rikka_model_core::{Follow, Reach, RigFacts};

use super::super::runtime::Puppet;
use super::super::text::PmxEncoding;
use super::soft_body::PmxSoftBody;
use super::version::PmxVersion;
use crate::euler;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PmxIndexSizes {
    pub vertex: u8,
    pub texture: u8,
    pub material: u8,
    pub bone: u8,
    pub morph: u8,
    pub rigid_body: u8,
}

impl Default for PmxIndexSizes {
    fn default() -> Self {
        Self {
            vertex: 4,
            texture: 1,
            material: 1,
            bone: 2,
            morph: 2,
            rigid_body: 2,
        }
    }
}

impl PmxIndexSizes {
    #[must_use]
    pub fn fitting(
        vertices: usize,
        textures: usize,
        materials: usize,
        bones: usize,
        morphs: usize,
        rigid_bodies: usize,
    ) -> Self {
        Self {
            vertex: unsigned_width(vertices),
            texture: signed_width(textures),
            material: signed_width(materials),
            bone: signed_width(bones),
            morph: signed_width(morphs),
            rigid_body: signed_width(rigid_bodies),
        }
    }
}

fn unsigned_width(count: usize) -> u8 {
    match count {
        0..=0xff => 1,
        0x100..=0xffff => 2,
        _ => 4,
    }
}

fn signed_width(count: usize) -> u8 {
    match count {
        0..=0x7f => 1,
        0x80..=0x7fff => 2,
        _ => 4,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PmxWeight {
    One,
    #[default]
    Two,
    Four,
    Spherical,
    Quaternion,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxSpherical {
    pub vertex: u32,
    pub centre: Vec3,
    pub radius0: Vec3,
    pub radius1: Vec3,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PmxVertices {
    pub weights: Vec<PmxWeight>,
    pub additional_uvs: Vec<Vec<Vec4>>,
    pub spherical: Vec<PmxSpherical>,
    pub edge_scales: Vec<f32>,
}

impl PmxVertices {
    fn convert(&mut self, conversion: &Conversion) {
        for sphere in &mut self.spherical {
            sphere.centre = conversion.point(sphere.centre);
            sphere.radius0 = conversion.point(sphere.radius0);
            sphere.radius1 = conversion.point(sphere.radius1);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PmxSphereMode {
    #[default]
    None,
    Multiply,
    Add,
    SubTexture,
}

impl PmxSphereMode {
    pub(crate) fn from_byte(byte: u8) -> Self {
        match byte {
            1 => Self::Multiply,
            2 => Self::Add,
            3 => Self::SubTexture,
            _ => Self::None,
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Multiply => 1,
            Self::Add => 2,
            Self::SubTexture => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PmxToon {
    Shared(u8),
    Texture(Option<usize>),
}

impl Default for PmxToon {
    fn default() -> Self {
        Self::Texture(None)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PmxMaterial {
    pub name: String,
    pub english_name: String,
    pub diffuse: Vec4,
    pub specular: Vec3,
    pub specular_power: f32,
    pub ambient: Vec3,
    pub flags: PmxMaterialFlags,
    pub edge_color: Vec4,
    pub edge_size: f32,
    pub texture: Option<usize>,
    pub sphere: Option<usize>,
    pub sphere_mode: PmxSphereMode,
    pub toon: PmxToon,
    pub memo: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PmxMaterialFlags(pub u8);

impl PmxMaterialFlags {
    pub const DOUBLE_SIDED: u8 = 0x01;
    pub const GROUND_SHADOW: u8 = 0x02;
    pub const CASTS_SHADOW: u8 = 0x04;
    pub const RECEIVES_SHADOW: u8 = 0x08;
    pub const OUTLINE: u8 = 0x10;
    pub const VERTEX_COLOR: u8 = 0x20;
    pub const POINTS: u8 = 0x40;
    pub const LINES: u8 = 0x80;
    pub(crate) const PMX_2_1: u8 = Self::VERTEX_COLOR | Self::POINTS | Self::LINES;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PmxBoneFlags(pub u16);

impl PmxBoneFlags {
    pub const TAIL_IS_BONE: u16 = 0x0001;
    pub const ROTATABLE: u16 = 0x0002;
    pub const TRANSLATABLE: u16 = 0x0004;
    pub const VISIBLE: u16 = 0x0008;
    pub const OPERABLE: u16 = 0x0010;
    pub const IK: u16 = 0x0020;
    pub const APPEND_LOCAL: u16 = 0x0080;
    pub const APPEND_ROTATE: u16 = 0x0100;
    pub const APPEND_TRANSLATE: u16 = 0x0200;
    pub const FIXED_AXIS: u16 = 0x0400;
    pub const LOCAL_AXES: u16 = 0x0800;
    pub const PHYSICS_AFTER: u16 = 0x1000;
    pub const EXTERNAL_PARENT: u16 = 0x2000;

    #[must_use]
    pub fn has(self, flag: u16) -> bool {
        self.0 & flag != 0
    }

    pub fn set(&mut self, flag: u16, enabled: bool) {
        match enabled {
            true => self.0 |= flag,
            false => self.0 &= !flag,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PmxTail {
    Offset(Vec3),
    Bone(Option<usize>),
}

impl Default for PmxTail {
    fn default() -> Self {
        Self::Offset(Vec3::ZERO)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxAppend {
    pub parent: usize,
    pub ratio: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxIkLink {
    pub bone: usize,
    pub limits: Option<PmxLimits>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PmxLimits {
    pub lower: Vec3,
    pub upper: Vec3,
}

impl PmxLimits {
    fn convert(&mut self, conversion: &Conversion) {
        let (lower, upper) = euler::angle_bounds(conversion, self.lower, self.upper);
        self.lower = lower;
        self.upper = upper;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PmxIk {
    pub target: usize,
    pub loops: u32,
    pub limit_angle: f32,
    pub links: Vec<PmxIkLink>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PmxBone {
    pub name: String,
    pub english_name: String,
    pub node: NodeId,
    pub position: Vec3,
    pub parent: Option<usize>,
    pub layer: i32,
    pub flags: PmxBoneFlags,
    pub tail: PmxTail,
    pub append: Option<PmxAppend>,
    pub fixed_axis: Option<Vec3>,
    pub local_axes: Option<(Vec3, Vec3)>,
    pub external_parent: Option<i32>,
    pub ik: Option<PmxIk>,
}

impl Default for PmxBone {
    fn default() -> Self {
        Self {
            name: String::new(),
            english_name: String::new(),
            node: NodeId(0),
            position: Vec3::ZERO,
            parent: None,
            layer: 0,
            flags: PmxBoneFlags::default(),
            tail: PmxTail::default(),
            append: None,
            fixed_axis: None,
            local_axes: None,
            external_parent: None,
            ik: None,
        }
    }
}

impl PmxBone {
    fn convert(&mut self, conversion: &Conversion) {
        self.position = conversion.point(self.position);
        if let PmxTail::Offset(offset) = &mut self.tail {
            *offset = conversion.point(*offset);
        }
        if let Some(axis) = &mut self.fixed_axis {
            *axis = conversion.vector(*axis);
        }
        if let Some((x, z)) = &mut self.local_axes {
            *x = conversion.vector(*x);
            *z = conversion.vector(*z);
        }
        for link in self.ik.iter_mut().flat_map(|ik| &mut ik.links) {
            if let Some(limits) = &mut link.limits {
                limits.convert(conversion);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PmxPanel {
    Hidden,
    Eyebrow,
    Eye,
    #[default]
    Mouth,
    Other,
}

impl PmxPanel {
    pub(crate) fn from_byte(byte: u8) -> Self {
        match byte {
            1 => Self::Eyebrow,
            2 => Self::Eye,
            3 => Self::Mouth,
            4 => Self::Other,
            _ => Self::Hidden,
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Self::Hidden => 0,
            Self::Eyebrow => 1,
            Self::Eye => 2,
            Self::Mouth => 3,
            Self::Other => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxGroupOffset {
    pub morph: usize,
    pub ratio: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxBoneOffset {
    pub bone: usize,
    pub translation: Vec3,
    pub rotation: Quat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxUvOffset {
    pub vertex: u32,
    pub offset: Vec4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxImpulseOffset {
    pub body: usize,
    pub local: bool,
    pub velocity: Vec3,
    pub torque: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PmxBlend {
    #[default]
    Multiply,
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmxMaterialOffset {
    pub material: Option<usize>,
    pub blend: PmxBlend,
    pub diffuse: Vec4,
    pub specular: Vec3,
    pub specular_power: f32,
    pub ambient: Vec3,
    pub edge_color: Vec4,
    pub edge_size: f32,
    pub texture_tint: Vec4,
    pub sphere_tint: Vec4,
    pub toon_tint: Vec4,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PmxOffsets {
    Group(Vec<PmxGroupOffset>),
    Vertex,
    Bone(Vec<PmxBoneOffset>),
    Uv {
        channel: u8,
        offsets: Vec<PmxUvOffset>,
    },
    Material(Vec<PmxMaterialOffset>),
    Flip(Vec<PmxGroupOffset>),
    Impulse(Vec<PmxImpulseOffset>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PmxMorph {
    pub name: String,
    pub english_name: String,
    pub panel: PmxPanel,
    pub offsets: PmxOffsets,
}

impl PmxMorph {
    fn convert(&mut self, conversion: &Conversion) {
        match &mut self.offsets {
            PmxOffsets::Bone(offsets) => {
                for offset in offsets {
                    offset.translation = conversion.point(offset.translation);
                    offset.rotation = conversion.rotation(offset.rotation);
                }
            }
            PmxOffsets::Impulse(offsets) => {
                for offset in offsets {
                    offset.velocity = conversion.point(offset.velocity);
                    offset.torque = euler::angles(conversion, offset.torque);
                }
            }
            PmxOffsets::Group(_)
            | PmxOffsets::Vertex
            | PmxOffsets::Uv { .. }
            | PmxOffsets::Material(_)
            | PmxOffsets::Flip(_) => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PmxDisplayTarget {
    Bone(usize),
    Morph(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PmxDisplayFrame {
    pub name: String,
    pub english_name: String,
    pub special: bool,
    pub targets: Vec<PmxDisplayTarget>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PmxShape {
    #[default]
    Sphere,
    Box,
    Capsule,
}

impl PmxShape {
    pub(crate) fn from_byte(byte: u8) -> Self {
        match byte {
            1 => Self::Box,
            2 => Self::Capsule,
            _ => Self::Sphere,
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Self::Sphere => 0,
            Self::Box => 1,
            Self::Capsule => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PmxPhysics {
    #[default]
    Static,
    Dynamic,
    DynamicOnBone,
}

impl PmxPhysics {
    pub(crate) fn from_byte(byte: u8) -> Self {
        match byte {
            1 => Self::Dynamic,
            2 => Self::DynamicOnBone,
            _ => Self::Static,
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Self::Static => 0,
            Self::Dynamic => 1,
            Self::DynamicOnBone => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PmxRigidBody {
    pub name: String,
    pub english_name: String,
    pub bone: Option<usize>,
    pub group: u8,
    pub collision_mask: u16,
    pub shape: PmxShape,
    pub size: Vec3,
    pub position: Vec3,
    pub rotation: Quat,
    pub mass: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub restitution: f32,
    pub friction: f32,
    pub physics: PmxPhysics,
}

impl PmxRigidBody {
    fn convert(&mut self, conversion: &Conversion) {
        self.size = conversion.scaling(self.size) * conversion.scale();
        self.position = conversion.point(self.position);
        self.rotation = conversion.rotation(self.rotation);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PmxJoint {
    pub name: String,
    pub english_name: String,
    pub kind: u8,
    pub bodies: (Option<usize>, Option<usize>),
    pub position: Vec3,
    pub rotation: Quat,
    pub translation_limits: PmxLimits,
    pub rotation_limits: PmxLimits,
    pub translation_spring: Vec3,
    pub rotation_spring: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PmxJointKind {
    #[default]
    Spring6Dof,
    SixDof,
    PointToPoint,
    ConeTwist,
    Slider,
    Hinge,
}

impl PmxJointKind {
    pub const ALL: [PmxJointKind; 6] = [
        PmxJointKind::Spring6Dof,
        PmxJointKind::SixDof,
        PmxJointKind::PointToPoint,
        PmxJointKind::ConeTwist,
        PmxJointKind::Slider,
        PmxJointKind::Hinge,
    ];

    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        Self::ALL.get(usize::from(byte)).copied()
    }

    #[must_use]
    pub fn byte(self) -> u8 {
        match self {
            Self::Spring6Dof => 0,
            Self::SixDof => 1,
            Self::PointToPoint => 2,
            Self::ConeTwist => 3,
            Self::Slider => 4,
            Self::Hinge => 5,
        }
    }
}

impl PmxJoint {
    fn convert(&mut self, conversion: &Conversion) {
        self.position = conversion.point(self.position);
        self.rotation = conversion.rotation(self.rotation);
        let (lower, upper) = euler::point_bounds(
            conversion,
            self.translation_limits.lower,
            self.translation_limits.upper,
        );
        self.translation_limits = PmxLimits { lower, upper };
        self.rotation_limits.convert(conversion);
        self.translation_spring = conversion.scaling(self.translation_spring);
        self.rotation_spring = conversion.scaling(self.rotation_spring);
    }
}

#[derive(Clone, Debug)]
pub struct Pmx {
    pub name: String,
    pub english_name: String,
    pub comment: String,
    pub english_comment: String,
    pub encoding: PmxEncoding,
    pub index_sizes: PmxIndexSizes,
    pub mesh: MeshId,
    pub root: NodeId,
    pub textures: Vec<String>,
    pub vertices: PmxVertices,
    pub materials: Vec<PmxMaterial>,
    pub bones: Vec<PmxBone>,
    pub morphs: Vec<PmxMorph>,
    pub display_frames: Vec<PmxDisplayFrame>,
    pub rigid_bodies: Vec<PmxRigidBody>,
    pub joints: Vec<PmxJoint>,
    pub soft_bodies: Vec<PmxSoftBody>,
}

impl Default for Pmx {
    fn default() -> Self {
        Self {
            name: String::new(),
            english_name: String::new(),
            comment: String::new(),
            english_comment: String::new(),
            encoding: PmxEncoding::default(),
            index_sizes: PmxIndexSizes::default(),
            mesh: MeshId(0),
            root: NodeId(0),
            textures: Vec::new(),
            vertices: PmxVertices::default(),
            materials: Vec::new(),
            bones: Vec::new(),
            morphs: Vec::new(),
            display_frames: Vec::new(),
            rigid_bodies: Vec::new(),
            joints: Vec::new(),
            soft_bodies: Vec::new(),
        }
    }
}

impl Extension for Pmx {
    const NAME: &'static str = "PMX";

    fn convert(&mut self, conversion: &Conversion) {
        self.vertices.convert(conversion);
        for bone in &mut self.bones {
            bone.convert(conversion);
        }
        for morph in &mut self.morphs {
            morph.convert(conversion);
        }
        for body in &mut self.rigid_bodies {
            body.convert(conversion);
        }
        for joint in &mut self.joints {
            joint.convert(conversion);
        }
        for soft_body in &mut self.soft_bodies {
            soft_body.convert(conversion);
        }
    }

    fn rig(&self, model: &Model) -> Option<Box<dyn Rig>> {
        Puppet::new(model).map(|puppet| Box::new(puppet) as Box<dyn Rig>)
    }

    fn describe_rig(&self, facts: &mut RigFacts) {
        let node = |index: usize| self.bones.get(index).map(|bone| bone.node);
        for bone in &self.bones {
            let appends = bone.flags.has(PmxBoneFlags::APPEND_ROTATE)
                || bone.flags.has(PmxBoneFlags::APPEND_TRANSLATE);
            if let Some(append) = bone.append.filter(|_| appends)
                && let Some(source) = node(append.parent)
            {
                facts.follows.push(Follow {
                    node: bone.node,
                    source,
                    ratio: append.ratio,
                });
            }
            if bone.flags.has(PmxBoneFlags::FIXED_AXIS) {
                facts.twisting.push(bone.node);
            }
            if let Some(ik) = &bone.ik
                && let Some(target) = node(ik.target)
            {
                facts.reaches.push(Reach {
                    target,
                    links: ik.links.iter().filter_map(|link| node(link.bone)).collect(),
                });
            }
        }
        for body in &self.rigid_bodies {
            if body.physics != PmxPhysics::Static
                && let Some(bone) = body.bone.and_then(node)
            {
                facts.swinging.push(bone);
            }
        }
    }

    fn alias_materials(&self, aliases: &mut Vec<MaterialAlias>) {
        for (index, material) in self.materials.iter().enumerate() {
            aliases.push(MaterialAlias {
                material: MaterialId(index as u32),
                name: material.english_name.clone(),
            });
        }
    }
}

impl Pmx {
    #[must_use]
    pub fn bone(&self, name: &str) -> Option<&PmxBone> {
        self.bones.iter().find(|bone| bone.name == name)
    }

    #[must_use]
    pub fn required_version(&self) -> PmxVersion {
        let quaternion = self.vertices.weights.contains(&PmxWeight::Quaternion);
        let morphs = self
            .morphs
            .iter()
            .any(|morph| matches!(morph.offsets, PmxOffsets::Flip(_) | PmxOffsets::Impulse(_)));
        let joints = self
            .joints
            .iter()
            .any(|joint| joint.kind != PmxJointKind::Spring6Dof.byte());
        let materials = self
            .materials
            .iter()
            .any(|material| material.flags.0 & PmxMaterialFlags::PMX_2_1 != 0);
        match quaternion || morphs || joints || materials || !self.soft_bodies.is_empty() {
            true => PmxVersion::V2_1,
            false => PmxVersion::V2_0,
        }
    }
}
