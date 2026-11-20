use rikka_renderer_3d::ShadowQuality;
use rikka_renderer_3d::glam::Vec3;
use rikka_shader_graph::NodeId;

use crate::asset::Asset;
use crate::name::Name;
use crate::world::choice::{Choice, choices};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LightKind {
    Directional,
    #[default]
    Point,
    Spot,
    Rect,
    Disc,
    Sphere,
    Tube,
}

choices!(LightKind, [
    LightKind::Directional => "directional",
    LightKind::Point => "point",
    LightKind::Spot => "spot",
    LightKind::Rect => "rect",
    LightKind::Disc => "disc",
    LightKind::Sphere => "sphere",
    LightKind::Tube => "tube",
]);

impl LightKind {
    #[must_use]
    pub fn is_area(self) -> bool {
        matches!(self, Self::Rect | Self::Disc | Self::Sphere | Self::Tube)
    }

    #[must_use]
    pub fn takes(self, unit: LightUnit) -> bool {
        match unit {
            LightUnit::Relative => true,
            LightUnit::Lux => self == Self::Directional,
            LightUnit::Candela => matches!(self, Self::Point | Self::Spot),
            LightUnit::Nits => self.is_area(),
            LightUnit::Lumens => self != Self::Directional,
        }
    }

    pub fn units(self) -> impl Iterator<Item = LightUnit> {
        LightUnit::ORDER
            .iter()
            .copied()
            .filter(move |unit| self.takes(*unit))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LightUnit {
    #[default]
    Relative,
    Lux,
    Candela,
    Lumens,
    Nits,
}

choices!(LightUnit, [
    LightUnit::Relative => "relative",
    LightUnit::Lux => "lux",
    LightUnit::Candela => "candela",
    LightUnit::Lumens => "lumens",
    LightUnit::Nits => "nits",
]);

impl LightUnit {
    #[must_use]
    pub fn span(self) -> (f32, f32) {
        match self {
            Self::Relative => (0.0, 20.0),
            Self::Lux => (0.0, 120_000.0),
            Self::Candela => (0.0, 5_000.0),
            Self::Lumens => (0.0, 20_000.0),
            Self::Nits => (0.0, 50_000.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LightAnchor {
    #[default]
    Camera,
    World,
}

choices!(LightAnchor, [
    LightAnchor::Camera => "camera",
    LightAnchor::World => "world",
]);

#[derive(Clone, Debug, PartialEq)]
pub struct LightLook {
    pub node: Vec<NodeId>,
    pub enabled: bool,
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
    pub unit: LightUnit,
    pub anchor: LightAnchor,
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub range: f32,
    pub inner: f32,
    pub outer: f32,
    pub width: f32,
    pub height: f32,
    pub radius: f32,
    pub length: f32,
    pub shadow: ShadowQuality,
    pub two_sided: bool,
    pub source_size: f32,
    pub profile: Option<Asset>,
    pub figures: Vec<Name>,
}

impl Default for LightLook {
    fn default() -> Self {
        Self {
            node: Vec::new(),
            enabled: true,
            kind: LightKind::Point,
            color: Vec3::new(1.0, 0.72, 0.5),
            intensity: 4.0,
            unit: LightUnit::Relative,
            anchor: LightAnchor::Camera,
            azimuth: 150f32.to_radians(),
            elevation: 30f32.to_radians(),
            distance: 2.5,
            range: 6.5,
            inner: 20f32.to_radians(),
            outer: 30f32.to_radians(),
            width: 1.0,
            height: 1.0,
            radius: 0.25,
            length: 1.0,
            shadow: ShadowQuality::None,
            two_sided: false,
            source_size: 0.0,
            profile: None,
            figures: Vec::new(),
        }
    }
}

impl LightLook {
    #[must_use]
    pub fn lights(&self, figure: &str) -> bool {
        self.figures.is_empty() || self.figures.iter().any(|name| name.as_str() == figure)
    }

    #[must_use]
    pub fn shines(&self) -> bool {
        self.enabled && self.intensity > 0.0
    }

    #[must_use]
    pub fn measured(&self) -> LightUnit {
        match self.kind.takes(self.unit) {
            true => self.unit,
            false => LightUnit::Relative,
        }
    }

    #[must_use]
    pub fn offset(&self, focus: Vec3, eye: Vec3) -> Vec3 {
        let front = match self.anchor {
            LightAnchor::Camera => {
                let towards = eye - focus;
                Vec3::new(towards.x, 0.0, towards.z).normalize_or(Vec3::Z)
            }
            LightAnchor::World => Vec3::Z,
        };
        let (sin, cos) = self.azimuth.sin_cos();
        let across = Vec3::new(
            front.x * cos + front.z * sin,
            0.0,
            front.z * cos - front.x * sin,
        );
        let (rise, flat) = self.elevation.sin_cos();
        across * flat + Vec3::Y * rise
    }
}
