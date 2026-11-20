mod photometric;
mod profile;
mod profiles;
mod rig;

pub use photometric::LUX_PER_UNIT;
pub use profile::{LightProfileId, Photometry};
pub(crate) use profiles::LightProfilePool;
pub use rig::{KeyLight, LightRig};

use glam::{Quat, Vec3};

use crate::gpu::{GpuLight, NONE_INDEX, UNMAPPED_SHADOW, light_flags, light_kinds};

/// Light-linking channels a [`Light`] and a [`Draw`](crate::Draw) carry unless told otherwise.
///
/// A light shines on, and shadows, only draws whose channel mask shares a bit with its own.
pub const DEFAULT_LIGHT_CHANNELS: u32 = 1;

pub(crate) const SUN_ANGULAR_RADIUS: f32 = 0.004_65;

/// Shadow map resolution of one view (a cascade, cube face, spot frustum or figure map).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShadowQuality {
    None,
    Low,
    #[default]
    Medium,
    High,
}

impl ShadowQuality {
    pub(crate) fn resolution(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Low => 512,
            Self::Medium => 1024,
            Self::High => 2048,
        }
    }

    pub(crate) fn casts(self) -> bool {
        self != Self::None
    }
}

/// Where a light sits and how it emits.
///
/// Directions point the way light travels (a directional light's `direction` points down for a
/// noon sun; a rect or disc emits along `direction`). `range` bounds a local light's influence:
/// it reaches zero there and is culled beyond it. Area lights (`Rect`, `Disc`, `Sphere`,
/// `Tube`) are integrated with linearly transformed cosines, so their specular highlights and
/// soft diffuse falloff follow the emitter's shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    Directional {
        direction: Vec3,
    },
    Point {
        position: Vec3,
        range: f32,
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    },
    /// A `width` × `height` rectangle centred on `position`; `width` runs along `tangent`.
    Rect {
        position: Vec3,
        direction: Vec3,
        tangent: Vec3,
        width: f32,
        height: f32,
        range: f32,
    },
    Disc {
        position: Vec3,
        direction: Vec3,
        radius: f32,
        range: f32,
    },
    Sphere {
        position: Vec3,
        radius: f32,
        range: f32,
    },
    /// A capsule of `radius` around a segment of `length` along `axis`, centred on `position`.
    Tube {
        position: Vec3,
        axis: Vec3,
        length: f32,
        radius: f32,
        range: f32,
    },
}

impl LightKind {
    pub(crate) fn is_directional(&self) -> bool {
        matches!(self, Self::Directional { .. })
    }

    pub(crate) fn is_area(&self) -> bool {
        matches!(
            self,
            Self::Rect { .. } | Self::Disc { .. } | Self::Sphere { .. } | Self::Tube { .. }
        )
    }

    pub(crate) fn position(&self) -> Vec3 {
        match *self {
            Self::Directional { .. } => Vec3::ZERO,
            Self::Point { position, .. }
            | Self::Spot { position, .. }
            | Self::Rect { position, .. }
            | Self::Disc { position, .. }
            | Self::Sphere { position, .. }
            | Self::Tube { position, .. } => position,
        }
    }

    pub(crate) fn direction(&self) -> Vec3 {
        match *self {
            Self::Directional { direction }
            | Self::Spot { direction, .. }
            | Self::Rect { direction, .. }
            | Self::Disc { direction, .. } => direction,
            Self::Tube { axis, .. } => axis,
            Self::Point { .. } | Self::Sphere { .. } => Vec3::NEG_Y,
        }
    }

    pub(crate) fn range(&self) -> f32 {
        match *self {
            Self::Directional { .. } => 0.0,
            Self::Point { range, .. }
            | Self::Spot { range, .. }
            | Self::Rect { range, .. }
            | Self::Disc { range, .. }
            | Self::Sphere { range, .. }
            | Self::Tube { range, .. } => range,
        }
    }

    pub(crate) fn extent(&self) -> f32 {
        match *self {
            Self::Directional { .. } | Self::Point { .. } | Self::Spot { .. } => 0.0,
            Self::Rect { width, height, .. } => 0.5 * width.hypot(height),
            Self::Disc { radius, .. } | Self::Sphere { radius, .. } => radius,
            Self::Tube { length, radius, .. } => 0.5 * length + radius,
        }
    }
}

/// One light of a frame.
///
/// `intensity` is in renderer units, which [`LUX_PER_UNIT`] ties to photometry: illuminance
/// for directional lights, luminous intensity (candela) for point and spot lights and
/// luminance (nits) for area lights, each divided by [`LUX_PER_UNIT`]. [`Light::lux`],
/// [`Light::candela`], [`Light::lumens`] and [`Light::nits`] set it from SI values; the sky's
/// sun uses the same scale, so auto exposure treats both alike.
#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct Light {
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
    pub shadow: ShadowQuality,
    /// Light-linking mask; see [`DEFAULT_LIGHT_CHANNELS`].
    pub channels: u32,
    /// Whether a rect or disc emits from both faces.
    pub two_sided: bool,
    /// Size of the emitter for soft shadows: angular radius in radians for directional lights,
    /// radius in world units for point and spot lights. Area lights use their own extent.
    pub source_size: f32,
    /// Intensity distribution of a point or spot light; see [`Light::profile`].
    pub photometry: Option<Photometry>,
}

impl Light {
    fn of(kind: LightKind, shadow: ShadowQuality) -> Self {
        Self {
            kind,
            color: Vec3::ONE,
            intensity: 1.0,
            shadow,
            channels: DEFAULT_LIGHT_CHANNELS,
            two_sided: false,
            source_size: 0.0,
            photometry: None,
        }
    }

    /// A directional light with the real sun's angular radius as its source size.
    pub fn directional(direction: Vec3) -> Self {
        Self::of(
            LightKind::Directional {
                direction: direction.normalize_or(Vec3::NEG_Y),
            },
            ShadowQuality::Medium,
        )
        .source_size(SUN_ANGULAR_RADIUS)
    }

    pub fn point(position: Vec3, range: f32) -> Self {
        Self::of(LightKind::Point { position, range }, ShadowQuality::None)
    }

    pub fn spot(position: Vec3, direction: Vec3, range: f32, inner: f32, outer: f32) -> Self {
        Self::of(
            LightKind::Spot {
                position,
                direction: direction.normalize_or(Vec3::NEG_Y),
                range,
                inner_angle: inner,
                outer_angle: outer.max(inner + 1e-3),
            },
            ShadowQuality::None,
        )
    }

    /// A rectangle emitting along `direction`; `tangent` is made perpendicular to it.
    pub fn rect(
        position: Vec3,
        direction: Vec3,
        tangent: Vec3,
        width: f32,
        height: f32,
        range: f32,
    ) -> Self {
        let direction = direction.normalize_or(Vec3::NEG_Y);
        let tangent = (tangent - direction * tangent.dot(direction))
            .try_normalize()
            .unwrap_or_else(|| direction.any_orthonormal_vector());
        Self::of(
            LightKind::Rect {
                position,
                direction,
                tangent,
                width: width.max(0.0),
                height: height.max(0.0),
                range,
            },
            ShadowQuality::None,
        )
    }

    pub fn disc(position: Vec3, direction: Vec3, radius: f32, range: f32) -> Self {
        Self::of(
            LightKind::Disc {
                position,
                direction: direction.normalize_or(Vec3::NEG_Y),
                radius: radius.max(0.0),
                range,
            },
            ShadowQuality::None,
        )
    }

    pub fn sphere(position: Vec3, radius: f32, range: f32) -> Self {
        Self::of(
            LightKind::Sphere {
                position,
                radius: radius.max(0.0),
                range,
            },
            ShadowQuality::None,
        )
    }

    pub fn tube(position: Vec3, axis: Vec3, length: f32, radius: f32, range: f32) -> Self {
        Self::of(
            LightKind::Tube {
                position,
                axis: axis.normalize_or(Vec3::X),
                length: length.max(0.0),
                radius: radius.max(0.0),
                range,
            },
            ShadowQuality::None,
        )
    }

    pub fn color(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn shadow(mut self, quality: ShadowQuality) -> Self {
        self.shadow = quality;
        self
    }

    pub fn channels(mut self, channels: u32) -> Self {
        self.channels = channels;
        self
    }

    pub fn two_sided(mut self, two_sided: bool) -> Self {
        self.two_sided = two_sided;
        self
    }

    pub fn source_size(mut self, size: f32) -> Self {
        self.source_size = size.max(0.0);
        self
    }

    /// Shapes a point or spot light by a profile, its nadir pointing down (−Y) for a point
    /// light and along the direction of a spot light.
    pub fn profile(self, profile: LightProfileId) -> Self {
        let orientation = match self.kind {
            LightKind::Spot { direction, .. } => Quat::from_rotation_arc(Vec3::NEG_Y, direction),
            _ => Quat::IDENTITY,
        };
        self.oriented_profile(profile, orientation)
    }

    /// Shapes a point or spot light by a profile turned by `orientation`; see [`Photometry`].
    pub fn oriented_profile(mut self, profile: LightProfileId, orientation: Quat) -> Self {
        self.photometry = Some(Photometry {
            profile,
            orientation: orientation.normalize(),
        });
        self
    }

    pub(crate) fn shadow_views(&self, cascades: u32) -> u32 {
        if !self.shadow.casts() {
            return 0;
        }
        match self.kind {
            LightKind::Directional { .. } => cascades,
            LightKind::Spot { .. } => 1,
            _ => 6,
        }
    }

    pub(crate) fn penumbra_size(&self) -> f32 {
        match self.kind.is_area() {
            true => self.kind.extent(),
            false => self.source_size,
        }
    }

    pub(crate) fn to_gpu(self, placed: Placed, profile: u32) -> GpuLight {
        let kind = self.kind;
        let (mut cone_scale, mut cone_offset, mut tangent, mut extent) =
            (0.0, 0.0, Vec3::ZERO, [0.0; 2]);
        let code = match kind {
            LightKind::Directional { .. } => light_kinds::DIRECTIONAL,
            LightKind::Point { .. } => light_kinds::POINT,
            LightKind::Spot {
                inner_angle,
                outer_angle,
                ..
            } => {
                let cos_inner = inner_angle.cos();
                let cos_outer = outer_angle.cos();
                cone_scale = 1.0 / (cos_inner - cos_outer).max(1e-4);
                cone_offset = -cos_outer * cone_scale;
                light_kinds::SPOT
            }
            LightKind::Rect {
                tangent: along,
                width,
                height,
                ..
            } => {
                tangent = along;
                extent = [0.5 * width, 0.5 * height];
                light_kinds::RECT
            }
            LightKind::Disc { radius, .. } => {
                tangent = kind.direction().any_orthonormal_vector();
                extent = [radius, radius];
                light_kinds::DISC
            }
            LightKind::Sphere { radius, .. } => {
                extent = [radius, radius];
                light_kinds::SPHERE
            }
            LightKind::Tube { length, radius, .. } => {
                extent = [0.5 * length, radius];
                light_kinds::TUBE
            }
        };
        let flags = placed.flags
            | match self.two_sided {
                true => light_flags::TWO_SIDED,
                false => 0,
            };
        let range = match kind.is_area() {
            true => kind.range().max(kind.extent()) + kind.extent(),
            false => kind.range(),
        };
        let mut direction = kind.direction();
        if let (Some(photometry), true) = (self.photometry, profile != NONE_INDEX) {
            if let LightKind::Point { .. } = kind {
                direction = photometry.orientation * Vec3::NEG_Y;
            }
            let zero = photometry.orientation * Vec3::X;
            tangent = (zero - direction * zero.dot(direction))
                .try_normalize()
                .unwrap_or_else(|| direction.any_orthonormal_vector());
        }
        GpuLight {
            position: kind.position().to_array(),
            range,
            direction: direction.to_array(),
            intensity: self.intensity,
            color: self.color.to_array(),
            kind: code,
            tangent: tangent.to_array(),
            cone_scale,
            extent,
            cone_offset,
            shadow: match (placed.shadow_count, placed.shadow & UNMAPPED_SHADOW) {
                (0, 0) => NONE_INDEX,
                _ => placed.shadow,
            },
            shadow_count: placed.shadow_count,
            characters: placed.characters,
            character_count: placed.character_count,
            channels: self.channels,
            figure: placed.figure,
            flags,
            size: self.penumbra_size(),
            profile,
        }
    }
}

/// Where a light's shadow views and owner landed in this frame.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Placed {
    pub shadow: u32,
    pub shadow_count: u32,
    pub characters: u32,
    pub character_count: u32,
    pub figure: u32,
    pub flags: u32,
}

impl Placed {
    pub(crate) fn unmapped(identity: u32) -> Self {
        Self {
            shadow: UNMAPPED_SHADOW | identity,
            ..Self::default()
        }
    }
}
