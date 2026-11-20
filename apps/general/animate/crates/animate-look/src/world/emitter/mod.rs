pub(crate) mod history;
pub(crate) mod nodes;
pub(crate) mod read;
mod render;

use rikka_renderer_3d::glam::{EulerRot, Mat4, Quat, Vec3};
use rikka_renderer_3d::{
    Attractor, Coloring, Ground, Launch, ParticleBlend, ParticleShape, Stretch, Turbulence, Twinkle,
};
use rikka_shader_graph::NodeId;

use crate::asset::Asset;
use crate::world::choice::choices;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub scale: Vec3,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            scale: Vec3::ONE,
        }
    }
}

impl Placement {
    #[must_use]
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            self.scale,
            Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, self.roll),
            self.position,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub start: f32,
    pub end: f32,
}

impl Timing {
    #[must_use]
    pub fn end(&self) -> f32 {
        match self.end > self.start {
            true => self.end,
            false => f32::INFINITY,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmitterLook {
    pub node: Vec<NodeId>,
    pub source: EmitterSource,
    pub placement: Placement,
    pub track: Vec<(f32, Placement)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EmitterSource {
    Preset(PresetEmitter),
    Custom(Box<CustomEmitter>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EmitterPreset {
    Rain,
    Snow,
    Sparks,
    Embers,
    Petals,
    Fireflies,
    Dust,
    Magic,
    Confetti,
    Leaves,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spread {
    Area(Vec3),
    Radius(f32),
    Speed(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresetEmitter {
    pub preset: EmitterPreset,
    pub amount: f32,
    pub spread: Spread,
    pub wind: Vec3,
    pub tint: Vec3,
    pub timing: Timing,
    pub seed: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    #[default]
    Point,
    Sphere,
    Box,
    Disc,
    Cone,
    Line,
}

choices!(ShapeKind, [
    ShapeKind::Point => "point",
    ShapeKind::Sphere => "sphere",
    ShapeKind::Box => "box",
    ShapeKind::Disc => "disc",
    ShapeKind::Cone => "cone",
    ShapeKind::Line => "line",
]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Facing {
    #[default]
    Camera,
    Velocity,
    Horizontal,
    Axis,
    Tumble,
}

choices!(Facing, [
    Facing::Camera => "camera",
    Facing::Velocity => "velocity",
    Facing::Horizontal => "horizontal",
    Facing::Axis => "axis",
    Facing::Tumble => "tumble",
]);

choices!(ParticleShape, [
    ParticleShape::Soft => "soft",
    ParticleShape::Disc => "disc",
    ParticleShape::Streak => "streak",
    ParticleShape::Star => "star",
    ParticleShape::Petal => "petal",
    ParticleShape::Leaf => "leaf",
    ParticleShape::Flake => "flake",
    ParticleShape::Square => "square",
]);

choices!(ParticleBlend, [
    ParticleBlend::Alpha => "alpha",
    ParticleBlend::Additive => "additive",
    ParticleBlend::Premultiplied => "premultiplied",
]);

choices!(Coloring, [
    Coloring::OverAge => "over_age",
    Coloring::PerParticle => "per_particle",
]);

choices!(rikka_renderer_3d::GroundContact, [
    rikka_renderer_3d::GroundContact::Bounce => "bounce",
    rikka_renderer_3d::GroundContact::Die => "die",
]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeLook {
    pub kind: ShapeKind,
    pub radius: f32,
    pub extent: Vec3,
    pub angle: f32,
    pub length: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurstLook {
    pub count: u32,
    pub time: f32,
    pub spread: f32,
    pub repeats: u32,
    pub interval: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmissionLook {
    pub rate: f32,
    pub timing: Timing,
    pub lifetime: f32,
    pub lifetime_variance: f32,
    pub max_particles: u32,
    pub burst: BurstLook,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForcesLook {
    pub gravity: Vec3,
    pub drag: f32,
    pub drag_variance: f32,
    pub wind: Vec3,
    pub turbulence: Turbulence,
    pub attracts: bool,
    pub attractor: Attractor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundLook {
    pub enabled: bool,
    pub ground: Ground,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppearanceLook {
    pub particle: ParticleShape,
    pub size: f32,
    pub size_end: f32,
    pub size_variance: f32,
    pub aspect: f32,
    pub colors: [Vec3; 4],
    pub early: f32,
    pub late: f32,
    pub coloring: Coloring,
    pub opacity: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub emission: f32,
    pub emission_end: f32,
    pub twinkle: Twinkle,
    pub rotation: f32,
    pub spin: f32,
    pub spin_variance: f32,
    pub stretch: Stretch,
    pub facing: Facing,
    pub facing_axis: Vec3,
    pub softness: f32,
    pub blend: ParticleBlend,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpriteLook {
    pub texture: Option<Asset>,
    pub columns: u32,
    pub rows: u32,
    pub frame_rate: f32,
    pub random_start: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomEmitter {
    pub seed: u32,
    pub shape: ShapeLook,
    pub emission: EmissionLook,
    pub launch: Launch,
    pub forces: ForcesLook,
    pub ground: GroundLook,
    pub appearance: AppearanceLook,
    pub sprite: SpriteLook,
    pub lit: bool,
    pub translucency: f32,
    pub layers: u32,
    pub bloom: bool,
}
