use std::f32::consts::PI;

use crate::error::{Error, Result};
use crate::light::{Light, LightKind};

/// Lux in one renderer illuminance unit.
///
/// The sky's sun carries 4 units above the atmosphere, about the solar illuminance of
/// 128 000 lux, so one unit is 32 000 lux (and one candela at one metre is 1 / 32 000).
pub const LUX_PER_UNIT: f32 = 32_000.0;

impl Light {
    /// Sets a directional light's illuminance in lux.
    pub fn lux(self, lux: f32) -> Result<Self> {
        match self.kind {
            LightKind::Directional { .. } => Ok(self.intensity(lux / LUX_PER_UNIT)),
            _ => Err(mismatch("lux", "directional")),
        }
    }

    /// Sets a point or spot light's luminous intensity in candela.
    pub fn candela(self, candela: f32) -> Result<Self> {
        match self.kind {
            LightKind::Point { .. } | LightKind::Spot { .. } => {
                Ok(self.intensity(candela / LUX_PER_UNIT))
            }
            _ => Err(mismatch("candela", "point and spot")),
        }
    }

    /// Sets an area light's luminance in nits (candela per square metre).
    pub fn nits(self, nits: f32) -> Result<Self> {
        match self.kind.is_area() {
            true => Ok(self.intensity(nits / LUX_PER_UNIT)),
            false => Err(mismatch("nits", "area")),
        }
    }

    /// Sets a local light's luminous power in lumens.
    ///
    /// A point light spreads it over the sphere; a spot light over its cone, so narrowing the
    /// cone brightens it; an area light over its emitting surface (both faces when two-sided)
    /// as a Lambertian emitter.
    pub fn lumens(self, lumens: f32) -> Result<Self> {
        let candela = match self.kind {
            LightKind::Directional { .. } => return Err(mismatch("lumens", "local")),
            LightKind::Point { .. } => lumens / (4.0 * PI),
            LightKind::Spot { outer_angle, .. } => {
                lumens / (2.0 * PI * (1.0 - outer_angle.cos()).max(1e-4))
            }
            _ => {
                let nits = lumens / (PI * self.emitting_area().max(1e-8));
                return Ok(self.intensity(nits / LUX_PER_UNIT));
            }
        };
        Ok(self.intensity(candela / LUX_PER_UNIT))
    }

    fn emitting_area(&self) -> f32 {
        let faces = match self.two_sided {
            true => 2.0,
            false => 1.0,
        };
        match self.kind {
            LightKind::Rect { width, height, .. } => width * height * faces,
            LightKind::Disc { radius, .. } => PI * radius * radius * faces,
            LightKind::Sphere { radius, .. } => 4.0 * PI * radius * radius,
            LightKind::Tube { length, radius, .. } => {
                2.0 * PI * radius * length + 4.0 * PI * radius * radius
            }
            _ => 0.0,
        }
    }
}

fn mismatch(unit: &str, kinds: &str) -> Error {
    Error::Invalid(format!("{unit} only measure {kinds} lights"))
}
