use glam::Vec3;
use rikka_gpu::texture::TextureId;

use crate::look::Sky;

/// What lights a looked frame from afar and shows behind it.
///
/// `Sky` is the physical atmosphere with its sun and moon lights, `Image` an HDR panorama
/// (image-based lighting), `None` the frame's flat [`ambient`](crate::Frame::ambient).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Environment {
    None,
    Sky(Sky),
    Image(EnvironmentImage),
}

impl Default for Environment {
    fn default() -> Self {
        Self::Sky(Sky::default())
    }
}

impl Environment {
    #[must_use]
    pub fn sky(&self) -> Option<&Sky> {
        match self {
            Self::Sky(sky) => Some(sky),
            Self::None | Self::Image(_) => None,
        }
    }

    #[must_use]
    pub fn image(&self) -> Option<&EnvironmentImage> {
        match self {
            Self::Image(image) => Some(image),
            Self::None | Self::Sky(_) => None,
        }
    }
}

/// An equirectangular (latitude-longitude) HDR panorama lighting the scene.
///
/// `texture` holds linear radiance, made with [`Pixels::rgba16f`](rikka_gpu::Pixels::rgba16f)
/// or [`Pixels::rgba32f`](rikka_gpu::Pixels::rgba32f); its top row is straight up and its
/// centre column looks along −Z before `rotation` (yaw about +Y, radians) turns it.
///
/// A texel value `v` lights the scene as radiance `v · intensity · tint` in renderer units,
/// i.e. `v · intensity · tint ·` [`NITS_PER_RADIANCE_UNIT`](crate::NITS_PER_RADIANCE_UNIT)
/// cd/m². A panorama stored in absolute nits therefore wants
/// `intensity = 1 / NITS_PER_RADIANCE_UNIT`.
///
/// `background` blends the panorama over the frame's background (0 hides it, 1 shows it) and
/// `blur` (0–1) shows it as seen by a surface of that roughness. `sun_clamp` caps every texel's
/// luminance (in texel values, before `intensity`) for lighting only, so a light from
/// [`Panorama::sun`](crate::Panorama::sun) at the same threshold replaces the clipped energy.
///
/// The lighting is rebuilt when `texture`, `intensity`, `tint`, `sun_clamp` or `rotation`
/// change; new pixels need a new texture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvironmentImage {
    pub texture: TextureId,
    pub rotation: f32,
    pub intensity: f32,
    pub background: f32,
    pub blur: f32,
    pub tint: Vec3,
    pub sun_clamp: Option<f32>,
}

impl EnvironmentImage {
    #[must_use]
    pub fn new(texture: TextureId) -> Self {
        Self {
            texture,
            rotation: 0.0,
            intensity: 1.0,
            background: 1.0,
            blur: 0.0,
            tint: Vec3::ONE,
            sun_clamp: None,
        }
    }

    pub(crate) fn radiance_scale(&self) -> Vec3 {
        (self.tint * self.intensity).max(Vec3::ZERO)
    }
}
