use glam::Vec3;
use rikka_types::srgb;

use crate::draw::Frame;
use crate::fluid::LiquidStyle;
use crate::light::{Light, ShadowQuality};
use crate::look::atmosphere::transmittance;
use crate::look::gpu::{
    GpuFigure, GpuLook, GpuScene, GpuSky, GpuSpace, GpuTrace, sun_angular_radius,
};
use crate::look::{Look, Sky};
use crate::shadow::Key;

pub const NITS_PER_RADIANCE_UNIT: f32 = 32_000.0;
const SOLAR_LUX: f32 = 128_000.0;
pub(crate) const SOLAR_ILLUMINANCE: f32 = SOLAR_LUX / NITS_PER_RADIANCE_UNIT;
pub(crate) const LUNAR_ILLUMINANCE: f32 = 0.06;
pub(crate) const MOON_TINT: Vec3 = Vec3::new(0.75, 0.85, 1.0);
const FAINTEST_LIGHT: f32 = 1e-4;

#[derive(Default)]
pub(crate) struct Resolved {
    pub figures: Vec<GpuFigure>,
    pub lights: Vec<Light>,
    pub owners: Vec<u32>,
    pub keys: Vec<Key>,
    pub leading: Option<usize>,
    pub style: Option<LiquidStyle>,
    pub studio_shadow: Option<usize>,
}

impl Resolved {
    pub(crate) fn resolve(&mut self, frame: &Frame, look: Option<&Look>) -> GpuLook {
        self.figures.clear();
        self.lights.clear();
        self.owners.clear();
        self.keys.clear();
        self.style = None;
        self.figures.push(GpuFigure::NONE);
        self.figures.extend(
            frame
                .figures
                .iter()
                .map(|figure| GpuFigure::from_head(figure.head)),
        );
        let figure_count = self.figures.len() as u32;
        if let Some(sky) = look.and_then(|look| look.environment.sky()) {
            celestial_lights(sky, &mut self.lights);
        }
        self.studio_shadow = None;
        if let Some(shadow) = frame.editor.workbench.shadow {
            self.studio_shadow = Some(self.lights.len());
            self.lights.push(
                Light::directional(shadow.direction)
                    .intensity(0.0)
                    .shadow(ShadowQuality::High),
            );
        }
        self.lights.extend(frame.lights.iter().copied());
        self.owners.resize(self.lights.len(), 0);
        self.leading = self
            .lights
            .iter()
            .position(|light| light.kind.is_directional());
        for (index, figure) in frame.figures.iter().enumerate() {
            let id = index as u32 + 1;
            self.lights.extend(figure.rig.lights.iter().copied());
            self.owners.resize(self.lights.len(), id);
            let leading = self.leading.map(|leading| &self.lights[leading]);
            if let Some(light) = figure.rig.keyed(leading) {
                let turned = figure.rig.key.is_some_and(|key| key.direction.is_some());
                match leading {
                    Some(_) => self.keys.push(Key {
                        figure: id,
                        light,
                        turned,
                    }),
                    None => {
                        self.lights.push(light);
                        self.owners.push(id);
                    }
                }
            }
        }
        let Some(look) = look else {
            let mut gpu = GpuLook::default();
            gpu.scene.figure_count = figure_count;
            return gpu;
        };
        self.style = Some(look.liquid);
        gpu_look(look, figure_count)
    }
}

pub(crate) fn sun_color(sky: &Sky) -> Vec3 {
    transmittance(sky.to_sun(), sky.haze) * SOLAR_ILLUMINANCE * sky.sun
}

pub(crate) fn moon_color(sky: &Sky) -> Vec3 {
    transmittance(sky.to_moon(), sky.haze)
        * MOON_TINT
        * LUNAR_ILLUMINANCE
        * sky.moon
        * sky.moon_illumination()
}

fn celestial_lights(sky: &Sky, lights: &mut Vec<Light>) {
    let sun = sun_color(sky);
    let moon = moon_color(sky);
    let sun_leads = luminance(sun) >= luminance(moon);
    let start = lights.len();
    for (color, towards, leads) in [
        (sun, sky.to_sun(), sun_leads),
        (moon, sky.to_moon(), !sun_leads),
    ] {
        let strength = color.max_element();
        if strength <= FAINTEST_LIGHT {
            continue;
        }
        lights.push(
            Light::directional(-towards)
                .color(color / strength)
                .intensity(strength)
                .source_size(sun_angular_radius(sky))
                .shadow(match leads {
                    true => sky.shadows,
                    false => ShadowQuality::None,
                }),
        );
    }
    lights[start..].sort_by_key(|light| light.shadow == ShadowQuality::None);
}

fn gpu_look(look: &Look, figure_count: u32) -> GpuLook {
    let (styled, stylization, shade_softness, shadow_tint, ambient) = match look.liquid {
        LiquidStyle::Physical => (true, 0.0, 0.0, Vec3::ONE, 0.0),
        LiquidStyle::Plain => (false, 1.0, 0.0, Vec3::ONE, 0.0),
        LiquidStyle::Cel {
            stylization,
            shade_softness,
            shadow_tint,
            ambient,
        } => (true, stylization, shade_softness, shadow_tint, ambient),
    };
    GpuLook {
        sky: GpuSky::from_look(look),
        space: GpuSpace::from_look(look),
        scene: GpuScene {
            enabled: 1,
            figure_count,
            styled: u32::from(styled),
            stylization,
            shade_softness,
            shadow_tint: shadow_tint.to_array(),
            ambient,
            ..GpuScene::default()
        },
        trace: GpuTrace {
            indirect_strength: look.illumination.strength.max(0.0),
            reach: look.illumination.reach.max(0.01),
            ..GpuTrace::default()
        },
    }
}

pub(crate) fn luminance(color: Vec3) -> f32 {
    srgb::luminance(color.to_array())
}
