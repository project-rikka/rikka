use std::time::Duration;

use rikka_gpu::TextureId;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Background, Characters, Composite, DEFAULT_LIGHT_CHANNELS, Environment,
    EnvironmentImage, FACE_LAYER, Figure, Fog, HAIR_LAYER, Head, Illumination, KeyLight, Light,
    LightProfileId, LightRig, Lines, LiquidStyle, Look, Occlusion, Overrides, PanoramaSun,
    ParameterKind, ParameterSpec, Particles, Reflections, SEE_THROUGH_LAYER, ShadowQuality,
    TextureBinding, TextureKind, Upscale, Value, ViewportConfig,
};

use crate::asset::Asset;
use crate::error::Error;
use crate::world::World;
use crate::world::emitter::EmitterLook;
use crate::world::light::{LightKind, LightLook, LightUnit};
use crate::world::scene::{
    AntialiasingMode, Backdrop, Floor, Override, OverrideValue, PanoramaLook, Rig,
};

const LIGHTING_GAIN: f32 = 1.5 * std::f32::consts::PI;
const AMBIENT_SHARE: f32 = 1.0 / 6.0;
const MULTISAMPLES: u32 = 4;

pub trait Resolver {
    fn texture(&mut self, path: &str, kind: TextureKind) -> Option<TextureBinding>;

    fn panorama(&mut self, path: &str) -> Option<TextureId>;

    fn panorama_sun(&mut self, path: &str, threshold: f32) -> Option<PanoramaSun>;

    fn profile(&mut self, path: &str) -> Option<LightProfileId>;

    fn parameter(&mut self, name: &str) -> Option<ParameterSpec>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sunlight {
    pub toward: Vec3,
    pub color: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Linked<'a> {
    pub name: &'a str,
    pub channel: u32,
}

impl World {
    pub fn look(&self, resolver: &mut impl Resolver) -> Look {
        Look {
            environment: self.environment(resolver),
            fog: self.fog.unwrap_or(Fog {
                density: 0.0,
                ..Fog::default()
            }),
            overrides: overrides(
                self.overrides.iter().filter(|entry| entry.figure.is_none()),
                resolver,
            ),
            shadows: self.shadows,
            occlusion: self.occlusion.unwrap_or(Occlusion {
                strength: 0.0,
                ..Occlusion::default()
            }),
            reflections: self.reflections.unwrap_or(Reflections {
                strength: 0.0,
                ..Reflections::default()
            }),
            illumination: self.illumination.unwrap_or(Illumination {
                strength: 0.0,
                ..Illumination::default()
            }),
            denoise: self.denoise,
            lines: self.lines.unwrap_or(Lines {
                strength: 0.0,
                ..Lines::default()
            }),
            particles: self.particles.unwrap_or(Particles {
                amount: 0.0,
                ..Particles::default()
            }),
            emitters: self
                .emitters
                .iter()
                .map(|emitter| emitter.emitter(resolver))
                .collect(),
            composite: Composite::new(),
            characters: Characters {
                hair_layers: mask(self.characters.hair, HAIR_LAYER),
                face_layers: mask(self.characters.face, FACE_LAYER),
                see_through_layers: mask(self.characters.see_through, SEE_THROUGH_LAYER),
                ..Characters::default()
            },
            liquid: LiquidStyle::default(),
        }
    }

    fn environment(&self, resolver: &mut impl Resolver) -> Environment {
        match &self.backdrop {
            Some(Backdrop::Sky(sky)) => Environment::Sky(*sky),
            Some(Backdrop::Panorama(panorama)) => {
                panorama_image(panorama, resolver).map_or(Environment::None, Environment::Image)
            }
            Some(Backdrop::Color(_)) | None => Environment::None,
        }
    }

    #[must_use]
    pub fn has_sky(&self) -> bool {
        matches!(self.backdrop, Some(Backdrop::Sky(_)))
    }

    #[must_use]
    pub fn background(&self) -> Background {
        match &self.backdrop {
            Some(Backdrop::Color(color)) => Background::Color(*color),
            Some(Backdrop::Sky(_) | Backdrop::Panorama(_)) => Background::Sky,
            None => Background::Color(Vec3::ZERO),
        }
    }

    #[must_use]
    pub fn sunlight(&self) -> Option<Sunlight> {
        let light = self.stage_light.as_ref()?;
        (!self.has_sky()).then(|| Sunlight {
            toward: direction_of(light.azimuth, light.elevation),
            color: light.color * LIGHTING_GAIN,
        })
    }

    #[must_use]
    pub fn ambient(&self) -> Vec3 {
        self.stage_light
            .map_or(Vec3::ZERO, |light| light.color * AMBIENT_SHARE)
    }

    #[must_use]
    pub fn floor(&self) -> Option<&Floor> {
        self.floor.as_ref()
    }

    #[must_use]
    pub fn grid(&self) -> bool {
        self.floor.is_some_and(|floor| floor.grid)
    }

    #[must_use]
    pub fn configure(&self, config: ViewportConfig) -> ViewportConfig {
        let sampling = &self.sampling;
        let sharpness = sampling.sharpness;
        match sampling.antialiasing {
            AntialiasingMode::Multisample => config
                .samples(MULTISAMPLES)
                .antialiasing(Antialiasing::Multisample),
            AntialiasingMode::Temporal => config.samples(1).antialiasing(Antialiasing::Temporal),
            AntialiasingMode::Upscale => config.samples(1).antialiasing(Antialiasing::Upscale(
                Upscale::fixed(sampling.render_scale).sharpness(sharpness),
            )),
            AntialiasingMode::Dynamic => config.samples(1).antialiasing(Antialiasing::Upscale(
                Upscale::dynamic(Duration::from_secs_f32(sampling.frame_budget / 1000.0))
                    .sharpness(sharpness),
            )),
        }
    }

    pub fn lights(
        &self,
        focus: Vec3,
        eye: Vec3,
        figures: &[Linked<'_>],
        resolver: &mut impl Resolver,
    ) -> Vec<Result<Light, Error>> {
        let mut lights = Vec::new();
        if let Some(sun) = self.sunlight()
            && let Some((color, intensity)) = split(sun.color)
        {
            lights.push(Ok(Light::directional(sun.toward)
                .color(color)
                .intensity(intensity)
                .shadow(ShadowQuality::High)));
        }
        if let Some(Backdrop::Panorama(panorama)) = &self.backdrop
            && let Some(threshold) = panorama.sun_clamp()
            && let Some(path) = &panorama.image
            && let Some(image) = panorama_image(panorama, resolver)
            && let Some(sun) = resolver.panorama_sun(path, threshold)
        {
            lights.push(Ok(sun.light(&image).shadow(panorama.sun_shadow)));
        }
        for (index, light) in self.lights.iter().enumerate() {
            if !light.shines() {
                continue;
            }
            let channels = match light.figures.is_empty() {
                true => DEFAULT_LIGHT_CHANNELS,
                false => figures
                    .iter()
                    .filter(|figure| light.lights(figure.name))
                    .fold(0, |channels, figure| channels | figure.channel),
            };
            let profile = light
                .profile
                .as_ref()
                .and_then(|path| resolver.profile(path));
            lights.push(
                light
                    .light(focus, eye)
                    .map(|built| {
                        let built = built.channels(channels);
                        match profile {
                            Some(profile) => built.profile(profile),
                            None => built,
                        }
                    })
                    .map_err(|error| Error::Light {
                        light: index,
                        reason: error.to_string(),
                    }),
            );
        }
        lights
    }

    #[must_use]
    pub fn rig(&self, figure: &str) -> Option<&Rig> {
        self.rigs
            .iter()
            .rev()
            .find(|rig| rig.figure.is_some_and(|name| name.as_str() == figure))
    }

    pub fn overrides_of<'a>(
        &'a self,
        figure: Option<&'a str>,
    ) -> impl Iterator<Item = &'a Override> {
        self.overrides
            .iter()
            .filter(move |entry| entry.figure.map(|name| name.as_str()) == figure)
    }

    pub fn drawn_figure(
        &self,
        figure: &str,
        head: Option<Head>,
        resolver: &mut impl Resolver,
    ) -> Figure {
        let overrides = overrides(self.overrides_of(Some(figure)), resolver);
        let Some(rig) = self.rig(figure) else {
            return Figure {
                head,
                overrides,
                shadow: self.figure_shadow,
                ..Figure::default()
            };
        };
        let key = KeyLight {
            direction: rig.turns.then(|| direction_of(rig.azimuth, rig.elevation)),
            color: rig.tints.then_some(rig.color),
            intensity: rig.brightens.then_some(rig.intensity),
        };
        let keyed = key.direction.is_some() || key.color.is_some() || key.intensity.is_some();
        Figure {
            head,
            overrides,
            rig: LightRig {
                key: keyed.then_some(key),
                lights: Vec::new(),
            },
            shadow: rig.shadow,
        }
    }

    pub fn assets(&self) -> impl Iterator<Item = &Asset> {
        let panorama = match &self.backdrop {
            Some(Backdrop::Panorama(panorama)) => panorama.image.as_ref(),
            _ => None,
        };
        let textures = self
            .overrides
            .iter()
            .filter_map(|entry| match &entry.value {
                OverrideValue::Texture(path) => Some(path),
                OverrideValue::Number(_) => None,
            });
        panorama
            .into_iter()
            .chain(
                self.lights
                    .iter()
                    .filter_map(|light| light.profile.as_ref()),
            )
            .chain(textures)
            .chain(self.emitters.iter().filter_map(EmitterLook::sprite))
    }
}

impl LightLook {
    pub fn light(&self, focus: Vec3, eye: Vec3) -> rikka_renderer_3d::Result<Light> {
        let offset = self.offset(focus, eye);
        let position = focus + offset * self.distance;
        let toward = -offset;
        let across = Vec3::Y.cross(toward).normalize_or(Vec3::X);
        let light = match self.kind {
            LightKind::Directional => Light::directional(toward),
            LightKind::Point => Light::point(position, self.range),
            LightKind::Spot => Light::spot(position, toward, self.range, self.inner, self.outer),
            LightKind::Rect => Light::rect(
                position,
                toward,
                across,
                self.width,
                self.height,
                self.range,
            ),
            LightKind::Disc => Light::disc(position, toward, self.radius, self.range),
            LightKind::Sphere => Light::sphere(position, self.radius, self.range),
            LightKind::Tube => Light::tube(position, across, self.length, self.radius, self.range),
        }
        .color(self.color)
        .shadow(self.shadow)
        .two_sided(self.two_sided)
        .source_size(self.source_size);
        match self.measured() {
            LightUnit::Relative => Ok(light.intensity(self.intensity)),
            LightUnit::Lux => light.lux(self.intensity),
            LightUnit::Candela => light.candela(self.intensity),
            LightUnit::Lumens => light.lumens(self.intensity),
            LightUnit::Nits => light.nits(self.intensity),
        }
    }
}

impl OverrideValue {
    #[must_use]
    pub fn to_value(
        &self,
        spec: &ParameterSpec,
        texture: impl FnOnce(&str, TextureKind) -> Option<TextureBinding>,
    ) -> Option<Value> {
        let clamp = |value: f32| match spec.range {
            Some(range) => value.clamp(range.low, range.high),
            None => value,
        };
        match (self, &spec.kind) {
            (Self::Texture(path), ParameterKind::Texture { kind, .. }) => {
                Some(Value::Texture(texture(path, *kind)))
            }
            (Self::Texture(_), _) | (Self::Number(_), ParameterKind::Texture { .. }) => None,
            (Self::Number(number), kind) => {
                let at = |index: usize| clamp(number.resized(4).component(index));
                let first = clamp(number.component(0));
                Some(match kind {
                    ParameterKind::Float => Value::Float(first),
                    ParameterKind::Vec2 => Value::Vec2(Vec2::new(at(0), at(1))),
                    ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. } => {
                        Value::Vec3(Vec3::new(at(0), at(1), at(2)))
                    }
                    ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. } => {
                        Value::Vec4(Vec4::new(at(0), at(1), at(2), at(3)))
                    }
                    ParameterKind::Int => Value::Int(first.round() as i32),
                    ParameterKind::UInt => Value::UInt(first.round().max(0.0) as u32),
                    ParameterKind::Bool => Value::Bool(number.component(0) > 0.5),
                    ParameterKind::Enum { options } => {
                        let last = options.len().checked_sub(1)?;
                        let chosen = (number.component(0).round().max(0.0) as usize).min(last);
                        Value::Enum(options[chosen].clone())
                    }
                    ParameterKind::Texture { .. } => return None,
                })
            }
        }
    }
}

fn overrides<'a>(
    entries: impl Iterator<Item = &'a Override>,
    resolver: &mut impl Resolver,
) -> Overrides {
    let mut overrides = Overrides::new();
    for entry in entries {
        let Some(spec) = resolver.parameter(&entry.parameter) else {
            continue;
        };
        if let Some(value) = entry
            .value
            .to_value(&spec, |path, kind| resolver.texture(path, kind))
        {
            overrides.set(&entry.parameter, value);
        }
    }
    overrides
}

fn panorama_image(
    panorama: &PanoramaLook,
    resolver: &mut impl Resolver,
) -> Option<EnvironmentImage> {
    let texture = resolver.panorama(panorama.image.as_ref()?)?;
    Some(EnvironmentImage {
        texture,
        rotation: panorama.rotation,
        intensity: panorama.intensity,
        background: panorama.background,
        blur: panorama.blur,
        tint: panorama.tint,
        sun_clamp: panorama.sun_clamp(),
    })
}

fn mask(on: bool, layer: u32) -> u32 {
    match on {
        true => layer,
        false => 0,
    }
}

fn split(color: Vec3) -> Option<(Vec3, f32)> {
    let intensity = color.max_element();
    (intensity > 0.0).then(|| (color / intensity, intensity))
}

fn direction_of(azimuth: f32, elevation: f32) -> Vec3 {
    let (rise, across) = elevation.sin_cos();
    let (sin, cos) = azimuth.sin_cos();
    -Vec3::new(across * sin, rise, across * cos)
}
