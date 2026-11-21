use std::sync::OnceLock;

use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Denoise, Fog, Illumination, Lines, Occlusion, Particles, Reflections, ScreenShadows,
    ShadowQuality, Shadows, Sky, TextureKind,
};
use rikka_shader_graph::{
    Behavior, Category, Hint, Hosts, Library, NodeSpec, PortSpec, Symbol, ValueType,
};

use crate::error::Error;
use crate::world::choice::options;
use crate::world::emitter::nodes::{EMITTER, emitter_nodes};
use crate::world::light::LightLook;
use crate::world::scene::{CharacterPasses, Floor, PanoramaLook, Rig, Sampling, StageLight};

pub(crate) const ENVIRONMENT: &str = "environment";
pub(crate) const STAGE_LIGHT: &str = "stage_light";
pub(crate) const LIGHTS: &str = "lights";
pub(crate) const FOG: &str = "fog";
pub(crate) const SHADOWS: &str = "shadows";
pub(crate) const OCCLUSION: &str = "occlusion";
pub(crate) const REFLECTIONS: &str = "reflections";
pub(crate) const ILLUMINATION: &str = "illumination";
pub(crate) const DENOISE: &str = "denoise";
pub(crate) const LINES: &str = "lines";
pub(crate) const PARTICLES: &str = "particles";
pub(crate) const FLOOR: &str = "floor";
pub(crate) const SAMPLING: &str = "sampling";
pub(crate) const CHARACTERS: &str = "characters";
pub(crate) const CAMERA: &str = "camera";
pub(crate) const OVERRIDES: &str = "overrides";
pub(crate) const RIGS: &str = "rigs";
pub(crate) const EMITTERS: &str = "emitters";

pub(crate) const SKY: &str = "world.sky";
pub(crate) const PANORAMA: &str = "world.panorama";
pub(crate) const BACKDROP: &str = "world.backdrop";
pub(crate) const STAGE_LIGHT_NODE: &str = "world.stage_light";
pub(crate) const LIGHT_NODE: &str = "world.light";
pub(crate) const FOG_NODE: &str = "world.fog";
pub(crate) const SHADOWS_NODE: &str = "world.shadows";
pub(crate) const OCCLUSION_NODE: &str = "world.occlusion";
pub(crate) const REFLECTIONS_NODE: &str = "world.reflections";
pub(crate) const ILLUMINATION_NODE: &str = "world.illumination";
pub(crate) const DENOISE_NODE: &str = "world.denoise";
pub(crate) const LINES_NODE: &str = "world.lines";
pub(crate) const PARTICLES_NODE: &str = "world.particles";
pub(crate) const FLOOR_NODE: &str = "world.floor";
pub(crate) const SAMPLING_NODE: &str = "world.sampling";
pub(crate) const CHARACTERS_NODE: &str = "world.characters";
pub(crate) const CAMERA_NODE: &str = "world.camera";
pub(crate) const OVERRIDE_NODE: &str = "world.override";
pub(crate) const RIG_NODE: &str = "world.rig";

const PANORAMA_FILES: &[&str] = &["hdr", "exr"];
const PROFILE_FILES: &[&str] = &["ies"];

pub(crate) fn record_type(name: &'static str) -> ValueType {
    ValueType::Record(Symbol::fixed(name))
}

pub(crate) fn record(
    id: &'static str,
    title: &'static str,
    category: Category,
    doc: &'static str,
    output: &'static str,
    inputs: Vec<PortSpec>,
) -> NodeSpec {
    NodeSpec {
        id,
        title,
        category,
        doc,
        hosts: Hosts::WORLD,
        inputs,
        outputs: vec![PortSpec::output(output, record_type(output))],
        behavior: Behavior::Record,
    }
}

pub(crate) fn soft(name: &'static str, value: f32, low: f32, high: f32) -> PortSpec {
    PortSpec::number(name, value).soft(low, high)
}

pub(crate) fn ranged(name: &'static str, value: f32, low: f32, high: f32) -> PortSpec {
    PortSpec::number(name, value).range(low, high)
}

pub(crate) fn angle(name: &'static str, value: f32) -> PortSpec {
    PortSpec::number(name, value).hint(Hint::Angle)
}

pub(crate) fn whole(name: &'static str, value: u32) -> PortSpec {
    PortSpec::number(name, value as f32).hint(Hint::Integer)
}

pub(crate) fn color(name: &'static str, value: Vec3) -> PortSpec {
    PortSpec::color(name, value.x, value.y, value.z)
}

pub(crate) fn vector(name: &'static str, value: Vec3) -> PortSpec {
    PortSpec::vector(name, value.x, value.y, value.z)
}

fn text(name: &'static str) -> PortSpec {
    PortSpec::empty(name, ValueType::Texture).hint(Hint::Text)
}

fn file(name: &'static str, extensions: &'static [&'static str]) -> PortSpec {
    PortSpec::empty(name, ValueType::Texture).hint(Hint::File(extensions))
}

pub(crate) const FIGURE_SHADOW: ShadowQuality = ShadowQuality::High;

pub(crate) fn shadows_default() -> Shadows {
    Shadows {
        screen: ScreenShadows {
            strength: 0.0,
            ..ScreenShadows::default()
        },
        ..Shadows::default()
    }
}

pub(crate) fn lines_default() -> Lines {
    Lines {
        strength: 1.0,
        ..Lines::default()
    }
}

pub(crate) fn particles_default() -> Particles {
    Particles {
        amount: 0.5,
        ..Particles::default()
    }
}

fn environment_nodes() -> Vec<NodeSpec> {
    let sky = Sky::default();
    let panorama = PanoramaLook::default();
    vec![
        record(
            SKY,
            "Physical sky",
            Category::Environment,
            "A physical sky with sun, moon, stars and clouds at an hour, place and day; it lights the scene and shows behind it, and the stage light then only adds its ambient share.",
            ENVIRONMENT,
            vec![
                ranged("hour", sky.hour, 0.0, 24.0),
                ranged("clouds", sky.clouds, 0.0, 1.0),
                angle("heading", sky.heading),
                ranged("latitude", sky.latitude, -66.0, 66.0),
                ranged("day_of_year", sky.day, 0.0, 365.0),
                soft("haze", sky.haze, 0.0, 4.0),
                soft("sun_size", sky.sun_size, 0.1, 5.0),
                soft("sun", sky.sun, 0.0, 3.0),
                soft("moon", sky.moon, 0.0, 5.0),
                ranged("moon_phase", sky.moon_phase, 0.0, 1.0),
                soft("stars", sky.stars, 0.0, 3.0),
                soft("cloud_drift", sky.cloud_drift, 0.0, 10.0),
                color("ground", sky.ground),
                options("shadows", sky.shadows),
            ],
        ),
        record(
            PANORAMA,
            "Panorama",
            Category::Environment,
            "An equirectangular HDR image that lights the scene and shows behind it by background (0 hides it), turned by rotation and seen through blur. With sun on, every texel brighter than sun_threshold becomes one shadow-casting sun of sun_shadow quality.",
            ENVIRONMENT,
            vec![
                file("image", PANORAMA_FILES),
                angle("rotation", panorama.rotation),
                soft("intensity", panorama.intensity, 0.0, 8.0),
                ranged("background", panorama.background, 0.0, 1.0),
                ranged("blur", panorama.blur, 0.0, 1.0),
                color("tint", panorama.tint),
                PortSpec::toggle("sun", panorama.sun),
                soft("sun_threshold", panorama.sun_threshold, 0.0, 64.0),
                options("sun_shadow", panorama.sun_shadow),
            ],
        ),
        record(
            BACKDROP,
            "Backdrop",
            Category::Environment,
            "A flat background colour behind the stage, as in MMD. With no environment wired the background is black.",
            ENVIRONMENT,
            vec![color("color", Vec3::ONE)],
        ),
    ]
}

fn light_nodes() -> Vec<NodeSpec> {
    let stage = StageLight::default();
    let light = LightLook::default();
    vec![
        record(
            STAGE_LIGHT_NODE,
            "Stage light",
            Category::Light,
            "MMD's light: the leading shadow-casting directional light and its ambient share. Under a physical sky only the ambient share remains. Unwired, there is no stage light and no ambient.",
            STAGE_LIGHT,
            vec![
                color("color", stage.color),
                angle("azimuth", stage.azimuth),
                angle("elevation", stage.elevation),
            ],
        ),
        record(
            LIGHT_NODE,
            "Light",
            Category::Light,
            "A light placed by azimuth, elevation and distance around the focused figure, turning with the camera when anchored to it. intensity is in unit: lux for directional, candela for point and spot, nits for area lights, lumens for any local light, relative for any; a unit that does not measure the kind counts as relative. figures lists the figure names it lights, separated by commas; empty lights everyone.",
            "light",
            vec![
                PortSpec::toggle("enabled", light.enabled),
                options("kind", light.kind),
                color("color", light.color),
                soft("intensity", light.intensity, 0.0, 20.0),
                options("unit", light.unit),
                options("anchor", light.anchor),
                angle("azimuth", light.azimuth),
                angle("elevation", light.elevation),
                soft("distance", light.distance, 0.1, 50.0),
                soft("range", light.range, 0.1, 100.0),
                angle("inner", light.inner),
                angle("outer", light.outer),
                soft("width", light.width, 0.01, 20.0),
                soft("height", light.height, 0.01, 20.0),
                soft("radius", light.radius, 0.01, 5.0),
                soft("length", light.length, 0.01, 20.0),
                options("shadow", light.shadow),
                PortSpec::toggle("two_sided", light.two_sided),
                ranged("source_size", light.source_size, 0.0, 1.0),
                file("profile", PROFILE_FILES),
                text("figures"),
            ],
        ),
        record(
            RIG_NODE,
            "Figure key light",
            Category::Light,
            "Replaces the leading light for one figure, named by figure: its direction when turns is on, its colour when tints is on and its intensity when brightens is on, with its own shadow map of shadow quality.",
            "rig",
            {
                let rig = Rig::default();
                vec![
                    text("figure"),
                    PortSpec::toggle("turns", rig.turns),
                    angle("azimuth", rig.azimuth),
                    angle("elevation", rig.elevation),
                    PortSpec::toggle("tints", rig.tints),
                    color("color", rig.color),
                    PortSpec::toggle("brightens", rig.brightens),
                    soft("intensity", rig.intensity, 0.0, 20.0),
                    options("shadow", rig.shadow),
                ]
            },
        ),
    ]
}

fn render_nodes() -> Vec<NodeSpec> {
    let fog = Fog::default();
    let shadows = shadows_default();
    let occlusion = Occlusion::default();
    let reflections = Reflections::default();
    let illumination = Illumination::default();
    let denoise = Denoise::default();
    let lines = lines_default();
    let particles = particles_default();
    let sampling = Sampling::default();
    let characters = CharacterPasses::default();
    vec![
        record(
            FOG_NODE,
            "Fog",
            Category::Environment,
            "Height fog thinning with height, scattering light forward and into sun shafts. Unwired, there is no fog.",
            FOG,
            vec![
                soft("density", fog.density, 0.0, 0.05),
                color("color", fog.color),
                soft("height", fog.height, 1.0, 200.0),
                ranged("forward", fog.forward, 0.0, 0.95),
                soft("shafts", fog.shafts, 0.0, 4.0),
            ],
        ),
        record(
            SHADOWS_NODE,
            "Shadows",
            Category::Render,
            "How lights cast shadows, how sharp each figure's own shadow map is and how strongly contact shadows darken creases. A figure with its own map leaves the world cascades of the leading light, so its self-shadowing and the shadow it lays on the floor stay sharp at any camera distance; figures none keeps figures in the cascades. Unwired, these defaults apply: auto tier, 120 m, one sample, high figure maps, no contact shadows.",
            SHADOWS,
            vec![
                options("tier", shadows.tier),
                soft("distance", shadows.distance, 10.0, 500.0),
                whole("samples", shadows.samples),
                options("figures", FIGURE_SHADOW),
                ranged("contact", shadows.screen.strength, 0.0, 1.0),
                soft("contact_length", shadows.screen.thickness, 0.005, 0.3),
            ],
        ),
        record(
            OCCLUSION_NODE,
            "Contact darkening",
            Category::Render,
            "Ambient occlusion darkening where surfaces meet. Unwired, it is off.",
            OCCLUSION,
            vec![
                options("tier", occlusion.tier),
                soft("strength", occlusion.strength, 0.0, 2.0),
                soft("radius", occlusion.radius, 0.05, 3.0),
                whole("samples", occlusion.samples),
            ],
        ),
        record(
            REFLECTIONS_NODE,
            "Reflections",
            Category::Render,
            "Reflections of the scene in glossy surfaces up to roughness_limit. Unwired, they are off.",
            REFLECTIONS,
            vec![
                options("tier", reflections.tier),
                soft("strength", reflections.strength, 0.0, 2.0),
                ranged("roughness_limit", reflections.roughness_limit, 0.0, 1.0),
                whole("samples", reflections.samples),
            ],
        ),
        record(
            ILLUMINATION_NODE,
            "Bounced light",
            Category::Render,
            "Light bounced between surfaces within reach metres, resampled over frames and neighbours. Unwired, it is off.",
            ILLUMINATION,
            vec![
                options("tier", illumination.tier),
                soft("strength", illumination.strength, 0.0, 2.0),
                soft("reach", illumination.reach, 1.0, 50.0),
                whole("samples", illumination.samples),
                options("resampling", illumination.resampling),
            ],
        ),
        record(
            DENOISE_NODE,
            "Denoising",
            Category::Render,
            "How many frames the denoiser remembers and how many spatial passes it runs. Unwired, 32 frames and 3 passes.",
            DENOISE,
            vec![
                soft("history", denoise.history, 1.0, 64.0),
                whole("passes", denoise.passes),
            ],
        ),
        record(
            LINES_NODE,
            "Line art",
            Category::Render,
            "Outlines drawn from depth and crease edges, on figures and by scenery on the scenery. Unwired, there are no lines.",
            LINES,
            vec![
                ranged("strength", lines.strength, 0.0, 1.0),
                color("color", lines.color),
                soft("width", lines.width, 0.5, 4.0),
                soft("depth", lines.depth, 0.0, 4.0),
                soft("crease", lines.crease, 0.0, 4.0),
                ranged("scenery", lines.environment, 0.0, 1.0),
            ],
        ),
        record(
            PARTICLES_NODE,
            "Particles",
            Category::Environment,
            "Floating motes, petals, sparkles or snow drifting on wind around the camera. Unwired, there are none.",
            PARTICLES,
            vec![
                ranged("amount", particles.amount, 0.0, 1.0),
                options("kind", particles.kind),
                soft("size", particles.size, 0.2, 4.0),
                color("color", particles.color),
                soft("glow", particles.glow, 0.0, 5.0),
                vector("wind", particles.wind),
                soft("reach", particles.reach, 2.0, 30.0),
                soft("focus", particles.focus, 0.1, 30.0),
                whole("seed", particles.seed),
            ],
        ),
        record(
            FLOOR_NODE,
            "Floor",
            Category::Stage,
            "The stage floor: a shadow catcher, a plain or tiled floor or water, and the grid. Unwired, there is no floor and no grid.",
            FLOOR,
            {
                let floor = Floor::default();
                vec![
                    options("kind", floor.kind),
                    ranged("shadow", floor.shadow, 0.0, 1.0),
                    color("color", floor.color),
                    ranged("roughness", floor.roughness, 0.0, 1.0),
                    ranged("reflection", floor.reflection, 0.0, 1.0),
                    soft("tile_scale", floor.tile_scale, 0.1, 8.0),
                    color("water_color", floor.water_color),
                    soft("wave_height", floor.wave_height, 0.0, 2.0),
                    soft("wave_speed", floor.wave_speed, 0.0, 4.0),
                    PortSpec::toggle("grid", floor.grid),
                ]
            },
        ),
        record(
            SAMPLING_NODE,
            "Antialiasing",
            Category::Render,
            "How the viewport smooths edges: multisampling, temporal, or temporal upscaling from a fixed render_scale or to a frame_budget in milliseconds. Unwired, temporal antialiasing.",
            SAMPLING,
            vec![
                options("antialiasing", sampling.antialiasing),
                ranged("render_scale", sampling.render_scale, 0.25, 1.0),
                soft("frame_budget", sampling.frame_budget, 4.0, 50.0),
                ranged("sharpness", sampling.sharpness, 0.0, 1.0),
            ],
        ),
        record(
            CHARACTERS_NODE,
            "Character passes",
            Category::Render,
            "The figure passes for hair shadows, face shading and see-through hair. Unwired, all three are on.",
            CHARACTERS,
            vec![
                PortSpec::toggle("hair", characters.hair),
                PortSpec::toggle("face", characters.face),
                PortSpec::toggle("see_through", characters.see_through),
            ],
        ),
        record(
            CAMERA_NODE,
            "Camera",
            Category::Render,
            "With autofocus on, a depth of field in the composite focuses on the focused figure. Unwired, autofocus is on.",
            CAMERA,
            vec![PortSpec::toggle("autofocus", true)],
        ),
        record(
            OVERRIDE_NODE,
            "Material override",
            Category::Render,
            "Sets the material parameter named parameter on every material that declares it, or only on the figure named figure: to texture when one is set, else to value.",
            "override",
            vec![
                text("parameter"),
                text("figure"),
                PortSpec::dynamic("value", 0.0),
                PortSpec::empty("texture", ValueType::Texture)
                    .hint(Hint::Texture(TextureKind::Colour)),
            ],
        ),
    ]
}

#[must_use]
pub fn world_nodes() -> Vec<NodeSpec> {
    let mut nodes = environment_nodes();
    nodes.extend(light_nodes());
    nodes.extend(render_nodes());
    nodes.extend(emitter_nodes());
    nodes
}

#[must_use]
pub fn world_outputs() -> Vec<PortSpec> {
    let single = |name: &'static str| PortSpec::empty(name, record_type(name));
    vec![
        single(ENVIRONMENT),
        single(STAGE_LIGHT),
        PortSpec::empty(LIGHTS, record_type("light")).many(),
        single(FOG),
        single(SHADOWS),
        single(OCCLUSION),
        single(REFLECTIONS),
        single(ILLUMINATION),
        single(DENOISE),
        single(LINES),
        single(PARTICLES),
        PortSpec::empty(EMITTERS, record_type(EMITTER)).many(),
        single(FLOOR),
        single(SAMPLING),
        single(CHARACTERS),
        single(CAMERA),
        PortSpec::empty(OVERRIDES, record_type("override")).many(),
        PortSpec::empty(RIGS, record_type("rig")).many(),
    ]
}

pub fn library() -> Result<Library, Error> {
    let mut library =
        rikka_shader_compile::library().map_err(|error| Error::Library(Box::new(error)))?;
    library.extend(world_nodes());
    Ok(library.with_world(world_outputs()))
}

#[must_use]
pub fn shared_library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| library().expect("the built-in look nodes form a library"))
}
