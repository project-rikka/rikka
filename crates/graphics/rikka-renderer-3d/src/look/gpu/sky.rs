use bytemuck::{Pod, Zeroable};

use crate::gpu::NONE_INDEX;
use crate::look::resolve::{
    LUNAR_ILLUMINANCE, MOON_TINT, SOLAR_ILLUMINANCE, moon_color, sun_color,
};
use crate::look::{Environment, EnvironmentImage, Look};

use crate::light::SUN_ANGULAR_RADIUS;
use crate::look::Sky;

const REAL_SUN_SIZE: f32 = 0.266;
const MOON_TO_SUN_SIZE: f32 = 1.15;

pub(crate) fn sun_angular_radius(sky: &Sky) -> f32 {
    SUN_ANGULAR_RADIUS * sky.sun_size.max(0.0) / REAL_SUN_SIZE
}
pub(crate) const AERIAL_RANGE: f32 = 32.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuSky {
    pub to_sun: [f32; 3],
    pub sun_angular_radius: f32,
    pub sun_color: [f32; 3],
    pub haze: f32,
    pub to_moon: [f32; 3],
    pub moon_illumination: f32,
    pub moon_color: [f32; 3],
    pub stars: f32,
    pub ground: [f32; 3],
    pub clouds: f32,
    pub fog_color: [f32; 3],
    pub fog_density: f32,
    pub fog_height: f32,
    pub fog_forward: f32,
    pub fog_shafts: f32,
    pub cloud_drift: f32,
    pub transmittance_lut: u32,
    pub multiscatter_lut: u32,
    pub sky_view_lut: u32,
    pub aerial_lut: u32,
    pub environment_image: u32,
    pub environment_levels: u32,
    pub fog_volume: u32,
    pub physical: u32,
    pub sun_illuminance: [f32; 3],
    pub moon_angular_radius: f32,
    pub moon_illuminance: [f32; 3],
    pub aerial_range: f32,
    pub star_rotation: [f32; 9],
    pub fog_grid: [u32; 3],
    pub environment: u32,
    pub panorama: u32,
    pub panorama_background: f32,
    pub panorama_blur: f32,
    pub panorama_rotation: [f32; 2],
    pub panorama_scale: [f32; 3],
    pub panorama_clamp: f32,
}

impl Default for GpuSky {
    fn default() -> Self {
        Self {
            to_sun: [0.0, 1.0, 0.0],
            sun_angular_radius: 0.0,
            sun_color: [0.0; 3],
            haze: 1.0,
            to_moon: [0.0, -1.0, 0.0],
            moon_illumination: 0.0,
            moon_color: [0.0; 3],
            stars: 0.0,
            ground: [0.0; 3],
            clouds: 0.0,
            fog_color: [1.0; 3],
            fog_density: 0.0,
            fog_height: 1.0,
            fog_forward: 0.0,
            fog_shafts: 0.0,
            cloud_drift: 0.0,
            transmittance_lut: NONE_INDEX,
            multiscatter_lut: NONE_INDEX,
            sky_view_lut: NONE_INDEX,
            aerial_lut: NONE_INDEX,
            environment_image: NONE_INDEX,
            environment_levels: 0,
            fog_volume: NONE_INDEX,
            physical: 0,
            sun_illuminance: [0.0; 3],
            moon_angular_radius: 0.0,
            moon_illuminance: [0.0; 3],
            aerial_range: AERIAL_RANGE,
            star_rotation: glam::Mat3::IDENTITY.to_cols_array(),
            fog_grid: [1; 3],
            environment: 0,
            panorama: NONE_INDEX,
            panorama_background: 0.0,
            panorama_blur: 0.0,
            panorama_rotation: [1.0, 0.0],
            panorama_scale: [0.0; 3],
            panorama_clamp: f32::MAX,
        }
    }
}

const _: () = assert!(size_of::<GpuSky>() == 264);

impl GpuSky {
    pub(crate) fn from_look(look: &Look) -> Self {
        let fog = &look.fog;
        let fogged = Self {
            fog_color: fog.color.to_array(),
            fog_density: fog.density.max(0.0),
            fog_height: fog.height.max(0.1),
            fog_forward: fog.forward.clamp(-0.95, 0.95),
            fog_shafts: fog.shafts.max(0.0),
            ..Self::default()
        };
        match &look.environment {
            Environment::None => fogged,
            Environment::Sky(sky) => Self::from_sky(sky, fogged),
            Environment::Image(image) => Self::from_image(image, fogged),
        }
    }

    fn from_sky(sky: &Sky, fogged: Self) -> Self {
        let sun_angular_radius = sun_angular_radius(sky);
        Self {
            to_sun: sky.to_sun().to_array(),
            sun_angular_radius,
            sun_color: sun_color(sky).to_array(),
            haze: sky.haze.max(0.0),
            to_moon: sky.to_moon().to_array(),
            moon_illumination: sky.moon_illumination(),
            moon_color: moon_color(sky).to_array(),
            stars: sky.stars.max(0.0),
            ground: sky.ground.to_array(),
            clouds: sky.clouds.clamp(0.0, 1.0),
            cloud_drift: sky.cloud_drift,
            physical: 1,
            sun_illuminance: [SOLAR_ILLUMINANCE * sky.sun.max(0.0); 3],
            moon_angular_radius: sun_angular_radius * MOON_TO_SUN_SIZE,
            moon_illuminance: (MOON_TINT * LUNAR_ILLUMINANCE * sky.moon.max(0.0)).to_array(),
            star_rotation: sky.star_rotation().to_cols_array(),
            environment: 1,
            ..fogged
        }
    }

    fn from_image(image: &EnvironmentImage, fogged: Self) -> Self {
        let (sin, cos) = image.rotation.sin_cos();
        Self {
            environment: 1,
            panorama_background: image.background.clamp(0.0, 1.0),
            panorama_blur: image.blur.clamp(0.0, 1.0),
            panorama_rotation: [cos, sin],
            panorama_scale: image.radiance_scale().to_array(),
            panorama_clamp: image.sun_clamp.map_or(f32::MAX, |clamp| clamp.max(0.0)),
            ..fogged
        }
    }
}
