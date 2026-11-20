use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{FloorParams, ShadowQuality, Sky, WaterParams};
use rikka_shader_graph::Constant;

use crate::asset::Asset;
use crate::name::Name;
use crate::world::choice::choices;

#[derive(Clone, Debug, PartialEq)]
pub enum Backdrop {
    Sky(Sky),
    Panorama(PanoramaLook),
    Color(Vec3),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PanoramaLook {
    pub image: Option<Asset>,
    pub rotation: f32,
    pub intensity: f32,
    pub background: f32,
    pub blur: f32,
    pub tint: Vec3,
    pub sun: bool,
    pub sun_threshold: f32,
    pub sun_shadow: ShadowQuality,
}

impl Default for PanoramaLook {
    fn default() -> Self {
        Self {
            image: None,
            rotation: 0.0,
            intensity: 1.0,
            background: 1.0,
            blur: 0.0,
            tint: Vec3::ONE,
            sun: false,
            sun_threshold: 8.0,
            sun_shadow: ShadowQuality::High,
        }
    }
}

impl PanoramaLook {
    #[must_use]
    pub fn sun_clamp(&self) -> Option<f32> {
        self.sun.then_some(self.sun_threshold)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageLight {
    pub color: Vec3,
    pub azimuth: f32,
    pub elevation: f32,
}

impl Default for StageLight {
    fn default() -> Self {
        Self {
            color: Vec3::splat(154.0 / 255.0),
            azimuth: 45f32.to_radians(),
            elevation: 55f32.to_radians(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FloorKind {
    None,
    #[default]
    Shadow,
    Plain,
    Tiles,
    Water,
}

choices!(FloorKind, [
    FloorKind::None => "none",
    FloorKind::Shadow => "shadow",
    FloorKind::Plain => "plain",
    FloorKind::Tiles => "tiles",
    FloorKind::Water => "water",
]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Floor {
    pub kind: FloorKind,
    pub shadow: f32,
    pub color: Vec3,
    pub roughness: f32,
    pub reflection: f32,
    pub tile_scale: f32,
    pub water_color: Vec3,
    pub wave_height: f32,
    pub wave_speed: f32,
    pub grid: bool,
}

impl Default for Floor {
    fn default() -> Self {
        let floor = FloorParams::default();
        let water = WaterParams::default();
        Self {
            kind: FloorKind::Shadow,
            shadow: floor.shadow,
            color: floor.color,
            roughness: floor.roughness,
            reflection: floor.reflection,
            tile_scale: floor.scale,
            water_color: water.deep,
            wave_height: water.height,
            wave_speed: water.speed,
            grid: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AntialiasingMode {
    Multisample,
    #[default]
    Temporal,
    Upscale,
    Dynamic,
}

choices!(AntialiasingMode, [
    AntialiasingMode::Multisample => "multisample",
    AntialiasingMode::Temporal => "temporal",
    AntialiasingMode::Upscale => "upscale",
    AntialiasingMode::Dynamic => "dynamic",
]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampling {
    pub antialiasing: AntialiasingMode,
    pub render_scale: f32,
    pub frame_budget: f32,
    pub sharpness: f32,
}

impl Default for Sampling {
    fn default() -> Self {
        Self {
            antialiasing: AntialiasingMode::Temporal,
            render_scale: 0.67,
            frame_budget: 16.0,
            sharpness: 0.2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterPasses {
    pub hair: bool,
    pub face: bool,
    pub see_through: bool,
}

impl Default for CharacterPasses {
    fn default() -> Self {
        Self {
            hair: true,
            face: true,
            see_through: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OverrideValue {
    Number(Constant),
    Texture(Asset),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    pub parameter: Name,
    pub figure: Option<Name>,
    pub value: OverrideValue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    pub figure: Option<Name>,
    pub turns: bool,
    pub azimuth: f32,
    pub elevation: f32,
    pub tints: bool,
    pub color: Vec3,
    pub brightens: bool,
    pub intensity: f32,
    pub shadow: ShadowQuality,
}

impl Default for Rig {
    fn default() -> Self {
        Self {
            figure: None,
            turns: true,
            azimuth: 45f32.to_radians(),
            elevation: 45f32.to_radians(),
            tints: true,
            color: Vec3::ONE,
            brightens: true,
            intensity: 3.0,
            shadow: ShadowQuality::Medium,
        }
    }
}
