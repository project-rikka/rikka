use std::fmt::Write as _;

use rikka_color_lookup::ColorLookup;
use rikka_gpu::{Filtering, Pixels};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Result;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Background, BuiltinEffect, BuiltinTemplate, Camera, Composite, CompositeInput,
    DepthOfField, Draw, EffectInstance, Emitter, Environment, EnvironmentImage, Exposure, Frame,
    Glow, Grade, Indices, Lens, Light, Look, MaterialDesc, MaterialId, MaterialState,
    MaterialTemplate, MeshData, MeshId, MotionBlur, Panorama, RenderLayer, ShadowQuality,
    TextureBinding, Tier, Tonemap, Value, Viewport, ViewportConfig,
};

use crate::allocations::{Phase, counted};

pub(crate) const EXTENT: (u32, u32) = (1280, 720);

pub(crate) const FULL_HD: (u32, u32) = (1920, 1080);

const LUT_SIZE: usize = 33;

const FRAME_RATE: f32 = 60.0;

const GRID: i32 = 6;

const SPACING: f32 = 1.4;

const RINGS: u32 = 24;

const LIGHT_RING: u32 = 64;

const MULTISAMPLES: u32 = 4;

const PANORAMA: (u32, u32) = (2048, 1024);

const SUN_RADIANCE: f32 = 20_000.0;

const SUN_RADIUS: f32 = 0.6;

const SUN_THRESHOLD: f32 = 50.0;

const PAINTS: [[f32; 4]; 4] = [
    [0.8, 0.2, 0.2, 1.0],
    [0.2, 0.7, 0.3, 1.0],
    [0.2, 0.3, 0.8, 1.0],
    [0.9, 0.8, 0.3, 1.0],
];

#[derive(Clone, Copy, Debug)]
pub(crate) enum Surfacing {
    Skin,
    Glass,
}

pub(crate) struct Studio {
    viewport: Viewport,
    extent: (u32, u32),
    ball: MeshId,
    floor: MeshId,
    paints: Vec<MaterialId>,
    ground: MaterialId,
    look: Look,
    lit: bool,
    sun: Option<Light>,
    allocated: u64,
}

impl Studio {
    pub(crate) fn new(
        renderer: &mut Renderer,
        extent: (u32, u32),
        antialiasing: Antialiasing,
    ) -> Result<Studio> {
        let ball = sphere(renderer, 0.5)?;
        let floor = plane(renderer, 12.0)?;
        let paints = PAINTS
            .into_iter()
            .map(|color| paint(renderer, color))
            .collect::<Result<Vec<_>>>()?;
        let ground = paint(renderer, [0.5, 0.5, 0.5, 1.0])?;
        let samples = match antialiasing {
            Antialiasing::Multisample => MULTISAMPLES,
            Antialiasing::Temporal | Antialiasing::Upscale(_) => 1,
        };
        let viewport = renderer.three().create_viewport(
            ViewportConfig::new(extent.0, extent.1)
                .samples(samples)
                .antialiasing(antialiasing),
        )?;
        let mut look = Look::default();
        look.composite = chain(Vec::new());
        Ok(Studio {
            viewport,
            extent,
            ball,
            floor,
            paints,
            ground,
            look,
            lit: false,
            sun: None,
            allocated: 0,
        }
        .tiered(Tier::ScreenSpace))
    }

    pub(crate) fn tiered(mut self, tier: Tier) -> Studio {
        self.look.shadows.tier = tier;
        self.look.occlusion.tier = tier;
        self.look.reflections.tier = tier;
        self.look.illumination.tier = tier;
        self
    }

    pub(crate) fn effects(mut self, renderer: &mut Renderer) -> Result<Studio> {
        let mut effects = Vec::new();
        for builtin in [
            BuiltinEffect::Vignette,
            BuiltinEffect::FilmGrain,
            BuiltinEffect::LayerTint,
        ] {
            let id = renderer.three().create_effect(&builtin.effect())?;
            effects.push((builtin, EffectInstance::new(id)));
        }
        self.look.composite = chain(effects);
        Ok(self)
    }

    pub(crate) fn emitters(mut self) -> Studio {
        let calm = Vec3::new(0.4, 0.0, 0.2);
        let at = |x: f32, z: f32| Mat4::from_translation(Vec3::new(x, -0.5, z));
        self.look.emitters = vec![
            Emitter::rain(3.0, Vec3::new(6.0, 6.0, 6.0), calm),
            Emitter::snow(1.0, Vec3::new(5.0, 5.0, 5.0), calm),
            Emitter::petals(4.0, Vec3::new(4.0, 4.0, 4.0), calm),
            Emitter::leaves(4.0, Vec3::new(4.0, 4.0, 4.0), calm),
            Emitter::sparks(20.0, 5.0, calm).at(at(-2.0, 1.0)),
            Emitter::magic(50.0, 0.6, calm).at(at(2.0, -1.0)),
            Emitter::embers(50.0, 0.8, calm).at(at(0.0, 2.5)),
            Emitter::fireflies(40.0, Vec3::new(5.0, 1.5, 5.0), calm),
            Emitter::dust(20.0, 0.8, calm).at(at(1.5, 1.5)),
        ];
        self
    }

    pub(crate) fn lights(mut self) -> Studio {
        self.lit = true;
        self
    }

    pub(crate) fn environment(mut self, renderer: &mut Renderer) -> Result<Studio> {
        let (width, height) = PANORAMA;
        let pixels = panorama(width, height);
        let texture = renderer
            .uploads()
            .create(Pixels::rgba16f(width, height, &pixels))?;
        let image = EnvironmentImage {
            rotation: 0.4,
            sun_clamp: Some(SUN_THRESHOLD),
            ..EnvironmentImage::new(texture)
        };
        self.sun = Panorama::new(width, height, &pixels)?
            .sun(SUN_THRESHOLD)
            .map(|sun| sun.light(&image));
        self.look.environment = Environment::Image(image);
        Ok(self)
    }

    pub(crate) fn effect_pack(mut self, renderer: &mut Renderer) -> Result<Studio> {
        let lookup = renderer.uploads().create_color_lookup(
            &ColorLookup::parse(&identity_cube())
                .map_err(|error| rikka_renderer::Error::Invalid(error.to_string()))?,
        )?;
        let mut effects = Vec::new();
        for builtin in BuiltinEffect::ALL {
            let id = renderer.three().create_effect(&builtin.effect())?;
            let instance = match builtin {
                BuiltinEffect::ColorLookup => EffectInstance::new(id).set(
                    "table",
                    Value::Texture(Some(TextureBinding::new(lookup, Filtering::NearestClamp))),
                ),
                _ => EffectInstance::new(id),
            };
            effects.push((builtin, instance));
        }
        self.look.composite = chain(effects);
        Ok(self)
    }

    pub(crate) fn surfaced(
        mut self,
        renderer: &mut Renderer,
        surfacing: Surfacing,
    ) -> Result<Studio> {
        let template = |builtin| MaterialDesc::new(&MaterialTemplate::builtin(builtin));
        self.paints = PAINTS
            .into_iter()
            .map(|color| {
                let base = Vec4::from_array(color).truncate();
                let desc = match surfacing {
                    Surfacing::Skin => template(BuiltinTemplate::Skin)
                        .with("base_color", base)?
                        .with("subsurface_radius", 0.04)?,
                    Surfacing::Glass => template(BuiltinTemplate::Standard)
                        .preset("glass")?
                        .with("transmission_color", base.lerp(Vec3::ONE, 0.6))?
                        .with("specular_roughness", 0.15)?
                        .with("geometry_thickness", 0.6)?
                        .state(MaterialState::transparent()),
                };
                renderer.three().create_material(desc)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(self)
    }

    pub(crate) fn allocated(&self) -> u64 {
        self.allocated
    }

    pub(crate) fn draw(&mut self, renderer: &mut Renderer, at: usize, phase: Phase) -> Result<f32> {
        let time = at as f32 / FRAME_RATE;
        let orbit = 0.2 * time;
        let eye = Vec3::new(9.0 * orbit.sin(), 4.0, 9.0 * orbit.cos());
        let camera = Camera::look_at(
            eye,
            Vec3::ZERO,
            Vec3::Y,
            0.9,
            self.extent.0 as f32 / self.extent.1 as f32,
            0.1,
        );
        let mut frame = Frame::new(camera);
        frame
            .background(Background::Sky)
            .time(time)
            .look(self.look.clone());
        frame.light(
            Light::point(Vec3::new(0.0, 3.0, 0.0), 12.0)
                .intensity(40.0)
                .shadow(ShadowQuality::None),
        );
        if self.lit {
            lights(&mut frame, time);
        }
        if let Some(sun) = self.sun {
            frame.light(sun);
        }
        frame.draw(
            Draw::new(self.floor, self.ground).transform(Mat4::from_translation(-0.5 * Vec3::Y)),
        );
        for x in -GRID / 2..GRID / 2 {
            for z in -GRID / 2..GRID / 2 {
                let index = (x + z).rem_euclid(self.paints.len() as i32) as usize;
                let bob = (time + x as f32 + 0.5 * z as f32).sin() * 0.2;
                let at = Vec3::new(x as f32 * SPACING, bob, z as f32 * SPACING);
                frame.draw(
                    Draw::new(self.ball, self.paints[index])
                        .transform(Mat4::from_translation(at))
                        .object_id((index + 1) as u32),
                );
            }
        }
        let (rendered, allocated) = counted(phase, || {
            renderer.three().render(&mut self.viewport, &frame)
        });
        rendered?;
        self.allocated = allocated;
        Ok((self.extent.0 * self.extent.1) as f32)
    }
}

fn lights(frame: &mut Frame, time: f32) {
    for index in 0..LIGHT_RING {
        let angle = index as f32 / LIGHT_RING as f32 * std::f32::consts::TAU + 0.3 * time;
        let hue = Vec3::new(angle.cos(), (angle + 2.1).cos(), (angle + 4.2).cos()) * 0.5 + 0.5;
        frame.light(
            Light::point(Vec3::new(5.0 * angle.cos(), 0.6, 5.0 * angle.sin()), 2.5)
                .color(hue)
                .intensity(3.0),
        );
    }
    frame
        .light(
            Light::rect(
                Vec3::new(-3.0, 2.5, 0.0),
                Vec3::new(0.6, -1.0, 0.0),
                Vec3::Z,
                2.0,
                0.6,
                8.0,
            )
            .intensity(6.0)
            .shadow(ShadowQuality::Medium),
        )
        .light(
            Light::disc(
                Vec3::new(3.0, 2.0, 0.0),
                Vec3::new(-0.5, -1.0, 0.0),
                0.4,
                8.0,
            )
            .intensity(10.0),
        )
        .light(Light::sphere(Vec3::new(0.0, 1.5, 3.0), 0.2, 6.0).intensity(20.0))
        .light(Light::tube(Vec3::new(0.0, 2.2, -3.0), Vec3::X, 2.0, 0.05, 6.0).intensity(20.0));
    for corner in [Vec3::new(10.0, 2.0, 10.0), Vec3::new(-10.0, 2.0, 10.0)] {
        frame.light(
            Light::spot(corner, Vec3::NEG_Y, 3.0, 0.4, 0.7)
                .intensity(30.0)
                .shadow(ShadowQuality::Medium),
        );
    }
}

const SIZE: &str = "size";

fn chain(effects: Vec<(BuiltinEffect, EffectInstance)>) -> Composite {
    let (display, scene): (Vec<_>, Vec<_>) = effects
        .into_iter()
        .partition(|(builtin, _)| builtin.is_display_referred());
    let mut composite = Composite::new();
    composite.push(Exposure::default());
    composite.push(MotionBlur { amount: 0.5 });
    composite.push(DepthOfField {
        aperture: 2.8,
        focus: 6.0,
        ..DepthOfField::default()
    });
    for (builtin, instance) in scene {
        let node = composite.push(instance);
        if builtin.effect().inputs().any(|socket| socket == SIZE) {
            composite.nodes[node].inputs.push((
                SIZE.to_owned(),
                CompositeInput::Layer(RenderLayer::Occlusion),
            ));
        }
    }
    composite.push(Glow::default());
    composite.push(Grade::default());
    composite.push(Tonemap {
        preserve_hue: 0.5,
        ..Tonemap::default()
    });
    composite.push(Lens::default());
    for (_, instance) in display {
        composite.push(instance);
    }
    composite
}

fn panorama(width: u32, height: u32) -> Vec<f32> {
    let sun = Vec3::new(0.4, 0.5, -0.6).normalize();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let polar = std::f32::consts::PI * (y as f32 + 0.5) / height as f32;
        for x in 0..width {
            let azimuth = std::f32::consts::TAU * (x as f32 + 0.5) / width as f32;
            let direction = Vec3::new(
                polar.sin() * azimuth.sin(),
                polar.cos(),
                -polar.sin() * azimuth.cos(),
            );
            let radiance = match direction.y > 0.0 {
                true => {
                    let haze = Vec3::new(0.9, 0.85, 0.8);
                    let zenith = Vec3::new(0.25, 0.45, 0.9);
                    let banks = 0.5 + 0.5 * (azimuth * 7.0).sin() * (polar * 5.0).cos();
                    haze.lerp(zenith, direction.y.sqrt()) * (0.6 + 0.4 * banks)
                }
                false => Vec3::new(0.25, 0.22, 0.2) * (1.0 + 0.3 * (azimuth * 40.0).sin()),
            };
            let radiance = match direction.angle_between(sun) < SUN_RADIUS.to_radians() {
                true => Vec3::splat(SUN_RADIANCE),
                false => radiance,
            };
            pixels.extend(radiance.extend(1.0).to_array());
        }
    }
    pixels
}

fn identity_cube() -> String {
    let mut text = format!("LUT_3D_SIZE {LUT_SIZE}\n");
    let last = (LUT_SIZE - 1) as f32;
    for blue in 0..LUT_SIZE {
        for green in 0..LUT_SIZE {
            for red in 0..LUT_SIZE {
                writeln!(
                    text,
                    "{} {} {}",
                    red as f32 / last,
                    green as f32 / last,
                    blue as f32 / last
                )
                .expect("write to String");
            }
        }
    }
    text
}

fn paint(renderer: &mut Renderer, color: [f32; 4]) -> Result<MaterialId> {
    let desc = MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Standard))
        .with("base_color", Vec4::from_array(color).truncate())?
        .with("specular_roughness", 0.5)?
        .with("gloss.roughness", 0.3)?
        .with("gloss.reflection", 0.5)?;
    renderer.three().create_material(desc)
}

fn plane(renderer: &mut Renderer, half: f32) -> Result<MeshId> {
    let positions = [
        Vec3::new(-half, 0.0, -half),
        Vec3::new(-half, 0.0, half),
        Vec3::new(half, 0.0, half),
        Vec3::new(half, 0.0, -half),
    ];
    let normals = [Vec3::Y; 4];
    let tangents = [Vec4::new(1.0, 0.0, 0.0, 1.0); 4];
    let uvs = [Vec2::ZERO, Vec2::Y, Vec2::ONE, Vec2::X];
    let indices = [0u32, 1, 2, 0, 2, 3];
    renderer.three().create_mesh(&MeshData {
        positions: &positions,
        normals: &normals,
        tangents: &tangents,
        uvs: &uvs,
        indices: Some(Indices::U32(&indices)),
        ..MeshData::default()
    })
}

fn sphere(renderer: &mut Renderer, radius: f32) -> Result<MeshId> {
    let segments = RINGS * 2;
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for ring in 0..=RINGS {
        let polar = std::f32::consts::PI * ring as f32 / RINGS as f32;
        for segment in 0..=segments {
            let azimuth = std::f32::consts::TAU * segment as f32 / segments as f32;
            normals.push(Vec3::new(
                polar.sin() * azimuth.cos(),
                polar.cos(),
                polar.sin() * azimuth.sin(),
            ));
            uvs.push(Vec2::new(
                segment as f32 / segments as f32,
                ring as f32 / RINGS as f32,
            ));
        }
    }
    let positions: Vec<Vec3> = normals.iter().map(|normal| *normal * radius).collect();
    let tangents = vec![Vec4::new(1.0, 0.0, 0.0, 1.0); positions.len()];
    let row = segments + 1;
    let mut indices = Vec::new();
    for ring in 0..RINGS {
        for segment in 0..segments {
            let a = ring * row + segment;
            let b = a + row;
            indices.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    renderer.three().create_mesh(&MeshData {
        positions: &positions,
        normals: &normals,
        tangents: &tangents,
        uvs: &uvs,
        indices: Some(Indices::U32(&indices)),
        ..MeshData::default()
    })
}
