use std::f64::consts::{PI, TAU};

use glam::{DVec3, Quat, Vec3};

use crate::error::{Error, Result};
use crate::light::{Light, ShadowQuality};
use crate::look::EnvironmentImage;
use crate::look::resolve::luminance;

const CHANNELS: usize = 4;

/// The linear RGBA texels of an equirectangular panorama, row by row from the top, laid out as
/// [`EnvironmentImage`] samples its texture.
#[derive(Clone, Copy, Debug)]
pub struct Panorama<'a> {
    width: u32,
    height: u32,
    pixels: &'a [f32],
}

/// The dominant light of a panorama: `direction` points towards it in the panorama's own frame,
/// `color` has luminance 1, `illuminance` is what it casts on a facing surface in renderer
/// units (lux / [`LUX_PER_UNIT`](crate::LUX_PER_UNIT)) per unit of
/// [`EnvironmentImage::intensity`], and `angular_radius` is the radius in radians of a disc
/// spreading the light as much.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanoramaSun {
    pub direction: Vec3,
    pub color: Vec3,
    pub illuminance: f32,
    pub angular_radius: f32,
}

struct Texel {
    direction: DVec3,
    excess: DVec3,
    solid_angle: f64,
}

impl<'a> Panorama<'a> {
    pub fn new(width: u32, height: u32, pixels: &'a [f32]) -> Result<Self> {
        let expected = width as usize * height as usize * CHANNELS;
        if width == 0 || height == 0 || pixels.len() != expected {
            return Err(Error::Invalid(format!(
                "panorama of {} samples is not {width}x{height} RGBA",
                pixels.len()
            )));
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Gathers the energy of every texel brighter than `threshold` (luminance in texel values)
    /// into one light, the energy an [`EnvironmentImage`] with `sun_clamp: Some(threshold)`
    /// leaves out; `None` when no texel exceeds it.
    #[must_use]
    pub fn sun(&self, threshold: f32) -> Option<PanoramaSun> {
        let threshold = f64::from(threshold.max(0.0));
        let mut toward = DVec3::ZERO;
        let mut weight = 0.0;
        self.for_each_excess(threshold, |texel| {
            let strength = f64::from(luminance(texel.excess.as_vec3())) * texel.solid_angle;
            toward += texel.direction * strength;
            weight += strength;
        });
        if weight <= 0.0 {
            return None;
        }
        let direction = toward.normalize();
        let spread = (toward.length() / weight).clamp(0.0, 1.0);
        let mut irradiance = DVec3::ZERO;
        self.for_each_excess(threshold, |texel| {
            let facing = texel.direction.dot(direction).max(0.0);
            irradiance += texel.excess * (texel.solid_angle * facing);
        });
        let irradiance = irradiance.as_vec3();
        let illuminance = luminance(irradiance);
        if illuminance <= 0.0 {
            return None;
        }
        Some(PanoramaSun {
            direction: direction.as_vec3(),
            color: irradiance / illuminance,
            illuminance,
            angular_radius: (2.0 * spread - 1.0).clamp(-1.0, 1.0).acos() as f32,
        })
    }

    fn for_each_excess(&self, threshold: f64, mut visit: impl FnMut(Texel)) {
        let (width, height) = (self.width as usize, self.height as usize);
        let across = TAU / width as f64;
        for (y, row) in self.pixels.chunks_exact(width * CHANNELS).enumerate() {
            let top = PI * y as f64 / height as f64;
            let bottom = PI * (y + 1) as f64 / height as f64;
            let solid_angle = across * (top.cos() - bottom.cos());
            let polar = 0.5 * (top + bottom);
            for (x, texel) in row.as_chunks::<CHANNELS>().0.iter().enumerate() {
                let color = DVec3::new(finite(texel[0]), finite(texel[1]), finite(texel[2]));
                let bright = f64::from(luminance(color.as_vec3()));
                if bright <= threshold {
                    continue;
                }
                let azimuth = across * (x as f64 + 0.5) - PI;
                visit(Texel {
                    direction: DVec3::new(
                        polar.sin() * azimuth.sin(),
                        polar.cos(),
                        -polar.sin() * azimuth.cos(),
                    ),
                    excess: color * (1.0 - threshold / bright),
                    solid_angle,
                });
            }
        }
    }
}

impl PanoramaSun {
    /// The shadow-casting directional light standing in for this sun under `image`, turned by
    /// its rotation and scaled by its intensity and tint.
    pub fn light(&self, image: &EnvironmentImage) -> Light {
        let towards = Quat::from_rotation_y(image.rotation) * self.direction;
        let color = self.color * self.illuminance * image.radiance_scale();
        let strength = color.max_element();
        Light::directional(-towards)
            .color(color / strength.max(f32::MIN_POSITIVE))
            .intensity(strength)
            .source_size(self.angular_radius)
            .shadow(ShadowQuality::High)
    }
}

fn finite(sample: f32) -> f64 {
    match sample.is_finite() {
        true => f64::from(sample.max(0.0)),
        false => 0.0,
    }
}
