pub(crate) mod choice;
pub(crate) mod emitter;
mod light;
pub(crate) mod nodes;
mod read;
mod render;
mod scene;

pub use choice::Choice;
pub use emitter::nodes::POSITION as EMITTER_POSITION;
pub use emitter::{
    AppearanceLook, BurstLook, CustomEmitter, EmissionLook, EmitterLook, EmitterPreset,
    EmitterSource, Facing, ForcesLook, GroundLook, Placement, PresetEmitter, ShapeKind, ShapeLook,
    Spread, SpriteLook, Timing,
};
pub use light::{LightAnchor, LightKind, LightLook, LightUnit};
pub use nodes::{library, shared_library, world_nodes, world_outputs};
pub use render::{Linked, Resolver, Sunlight};
pub use scene::{
    AntialiasingMode, Backdrop, CharacterPasses, Floor, FloorKind, Override, OverrideValue,
    PanoramaLook, Rig, Sampling, StageLight,
};

use rikka_renderer_3d::{
    Denoise, Fog, Illumination, Lines, Occlusion, Particles, Reflections, ScreenShadows, Shadows,
    Sky,
};
use rikka_shader_graph::Record;

use crate::error::Error;
use crate::world::nodes::{
    BACKDROP, CAMERA, CHARACTERS, DENOISE, EMITTERS, ENVIRONMENT, FLOOR, FOG, ILLUMINATION, LIGHTS,
    LINES, OCCLUSION, OVERRIDES, PANORAMA, PARTICLES, REFLECTIONS, RIGS, SAMPLING, SHADOWS, SKY,
    STAGE_LIGHT, shadows_default,
};
use crate::world::read::Read;

#[derive(Clone, Debug, PartialEq)]
pub struct World {
    pub backdrop: Option<Backdrop>,
    pub stage_light: Option<StageLight>,
    pub lights: Vec<LightLook>,
    pub fog: Option<Fog>,
    pub shadows: Shadows,
    pub occlusion: Option<Occlusion>,
    pub reflections: Option<Reflections>,
    pub illumination: Option<Illumination>,
    pub denoise: Denoise,
    pub lines: Option<Lines>,
    pub particles: Option<Particles>,
    pub emitters: Vec<EmitterLook>,
    pub floor: Option<Floor>,
    pub sampling: Sampling,
    pub characters: CharacterPasses,
    pub autofocus: bool,
    pub overrides: Vec<Override>,
    pub rigs: Vec<Rig>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            backdrop: None,
            stage_light: None,
            lights: Vec::new(),
            fog: None,
            shadows: shadows_default(),
            occlusion: None,
            reflections: None,
            illumination: None,
            denoise: Denoise::default(),
            lines: None,
            particles: None,
            emitters: Vec::new(),
            floor: None,
            sampling: Sampling::default(),
            characters: CharacterPasses::default(),
            autofocus: true,
            overrides: Vec::new(),
            rigs: Vec::new(),
        }
    }
}

fn optional<T>(
    world: &Record,
    port: &str,
    read: impl FnOnce(Read<'_>) -> Result<T, Error>,
) -> Result<Option<T>, Error> {
    world
        .record(port)
        .map(|record| read(Read::new(record)))
        .transpose()
}

impl World {
    pub fn from_record(world: &Record) -> Result<Self, Error> {
        let base = Self::default();
        Ok(Self {
            backdrop: world.record(ENVIRONMENT).map(backdrop).transpose()?,
            stage_light: optional(world, STAGE_LIGHT, stage_light)?,
            lights: world
                .records(LIGHTS)
                .iter()
                .map(light)
                .collect::<Result<_, _>>()?,
            fog: optional(world, FOG, fog)?,
            shadows: optional(world, SHADOWS, shadows)?.unwrap_or(base.shadows),
            occlusion: optional(world, OCCLUSION, occlusion)?,
            reflections: optional(world, REFLECTIONS, reflections)?,
            illumination: optional(world, ILLUMINATION, illumination)?,
            denoise: optional(world, DENOISE, denoise)?.unwrap_or(base.denoise),
            lines: optional(world, LINES, lines)?,
            particles: optional(world, PARTICLES, particles)?,
            emitters: world
                .records(EMITTERS)
                .iter()
                .map(emitter::read::emitter)
                .collect::<Result<_, _>>()?,
            floor: optional(world, FLOOR, floor)?,
            sampling: optional(world, SAMPLING, sampling)?.unwrap_or(base.sampling),
            characters: optional(world, CHARACTERS, characters)?.unwrap_or(base.characters),
            autofocus: optional(world, CAMERA, |read| read.toggle("autofocus"))?
                .unwrap_or(base.autofocus),
            overrides: world
                .records(OVERRIDES)
                .iter()
                .map(|record| override_of(Read::new(record)))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect(),
            rigs: world
                .records(RIGS)
                .iter()
                .map(|record| rig(Read::new(record)))
                .collect::<Result<_, _>>()?,
        })
    }
}

fn backdrop(record: &Record) -> Result<Backdrop, Error> {
    let read = Read::new(record);
    Ok(match record.id.as_str() {
        SKY => Backdrop::Sky(Sky {
            hour: read.float("hour")?,
            clouds: read.float("clouds")?,
            heading: read.float("heading")?,
            latitude: read.float("latitude")?,
            day: read.float("day_of_year")?,
            haze: read.float("haze")?,
            sun_size: read.float("sun_size")?,
            sun: read.float("sun")?,
            moon: read.float("moon")?,
            moon_phase: read.float("moon_phase")?,
            stars: read.float("stars")?,
            cloud_drift: read.float("cloud_drift")?,
            ground: read.vec3("ground")?,
            shadows: read.choice("shadows")?,
        }),
        PANORAMA => Backdrop::Panorama(PanoramaLook {
            image: read.asset("image")?,
            rotation: read.float("rotation")?,
            intensity: read.float("intensity")?,
            background: read.float("background")?,
            blur: read.float("blur")?,
            tint: read.vec3("tint")?,
            sun: read.toggle("sun")?,
            sun_threshold: read.float("sun_threshold")?,
            sun_shadow: read.choice("sun_shadow")?,
        }),
        BACKDROP => Backdrop::Color(read.vec3("color")?),
        other => {
            return Err(Error::Record {
                node: other.to_owned(),
                port: ENVIRONMENT.to_owned(),
                expected: "a sky, panorama or backdrop",
            });
        }
    })
}

fn stage_light(read: Read<'_>) -> Result<StageLight, Error> {
    Ok(StageLight {
        color: read.vec3("color")?,
        azimuth: read.float("azimuth")?,
        elevation: read.float("elevation")?,
    })
}

fn light(record: &Record) -> Result<LightLook, Error> {
    let read = Read::new(record);
    Ok(LightLook {
        node: record.path.clone(),
        enabled: read.toggle("enabled")?,
        kind: read.choice("kind")?,
        color: read.vec3("color")?,
        intensity: read.float("intensity")?,
        unit: read.choice("unit")?,
        anchor: read.choice("anchor")?,
        azimuth: read.float("azimuth")?,
        elevation: read.float("elevation")?,
        distance: read.float("distance")?,
        range: read.float("range")?,
        inner: read.float("inner")?,
        outer: read.float("outer")?,
        width: read.float("width")?,
        height: read.float("height")?,
        radius: read.float("radius")?,
        length: read.float("length")?,
        shadow: read.choice("shadow")?,
        two_sided: read.toggle("two_sided")?,
        source_size: read.float("source_size")?,
        profile: read.asset("profile")?,
        figures: read.names("figures")?,
    })
}

fn fog(read: Read<'_>) -> Result<Fog, Error> {
    Ok(Fog {
        density: read.float("density")?,
        color: read.vec3("color")?,
        height: read.float("height")?,
        forward: read.float("forward")?,
        shafts: read.float("shafts")?,
    })
}

fn shadows(read: Read<'_>) -> Result<Shadows, Error> {
    Ok(Shadows {
        tier: read.choice("tier")?,
        distance: read.float("distance")?,
        samples: read.whole("samples")?,
        screen: ScreenShadows {
            strength: read.float("contact")?,
            thickness: read.float("contact_length")?,
        },
    })
}

fn occlusion(read: Read<'_>) -> Result<Occlusion, Error> {
    Ok(Occlusion {
        tier: read.choice("tier")?,
        strength: read.float("strength")?,
        radius: read.float("radius")?,
        samples: read.whole("samples")?,
    })
}

fn reflections(read: Read<'_>) -> Result<Reflections, Error> {
    Ok(Reflections {
        tier: read.choice("tier")?,
        strength: read.float("strength")?,
        roughness_limit: read.float("roughness_limit")?,
        samples: read.whole("samples")?,
    })
}

fn illumination(read: Read<'_>) -> Result<Illumination, Error> {
    Ok(Illumination {
        tier: read.choice("tier")?,
        strength: read.float("strength")?,
        reach: read.float("reach")?,
        samples: read.whole("samples")?,
        resampling: read.choice("resampling")?,
    })
}

fn denoise(read: Read<'_>) -> Result<Denoise, Error> {
    Ok(Denoise {
        history: read.float("history")?,
        passes: read.whole("passes")?,
    })
}

fn lines(read: Read<'_>) -> Result<Lines, Error> {
    Ok(Lines {
        strength: read.float("strength")?,
        color: read.vec3("color")?,
        width: read.float("width")?,
        depth: read.float("depth")?,
        crease: read.float("crease")?,
        environment: read.float("scenery")?,
    })
}

fn particles(read: Read<'_>) -> Result<Particles, Error> {
    Ok(Particles {
        amount: read.float("amount")?,
        kind: read.choice("kind")?,
        size: read.float("size")?,
        color: read.vec3("color")?,
        glow: read.float("glow")?,
        wind: read.vec3("wind")?,
        reach: read.float("reach")?,
        focus: read.float("focus")?,
        seed: read.whole("seed")?,
    })
}

fn floor(read: Read<'_>) -> Result<Floor, Error> {
    Ok(Floor {
        kind: read.choice("kind")?,
        shadow: read.float("shadow")?,
        color: read.vec3("color")?,
        roughness: read.float("roughness")?,
        reflection: read.float("reflection")?,
        tile_scale: read.float("tile_scale")?,
        water_color: read.vec3("water_color")?,
        wave_height: read.float("wave_height")?,
        wave_speed: read.float("wave_speed")?,
        grid: read.toggle("grid")?,
    })
}

fn sampling(read: Read<'_>) -> Result<Sampling, Error> {
    Ok(Sampling {
        antialiasing: read.choice("antialiasing")?,
        render_scale: read.float("render_scale")?,
        frame_budget: read.float("frame_budget")?,
        sharpness: read.float("sharpness")?,
    })
}

fn characters(read: Read<'_>) -> Result<CharacterPasses, Error> {
    Ok(CharacterPasses {
        hair: read.toggle("hair")?,
        face: read.toggle("face")?,
        see_through: read.toggle("see_through")?,
    })
}

fn override_of(read: Read<'_>) -> Result<Option<Override>, Error> {
    let Some(parameter) = read.name("parameter")? else {
        return Ok(None);
    };
    let value = match read.asset("texture")? {
        Some(texture) => OverrideValue::Texture(texture),
        None => OverrideValue::Number(read.number("value")?),
    };
    Ok(Some(Override {
        parameter,
        figure: read.name("figure")?,
        value,
    }))
}

fn rig(read: Read<'_>) -> Result<Rig, Error> {
    Ok(Rig {
        figure: read.name("figure")?,
        turns: read.toggle("turns")?,
        azimuth: read.float("azimuth")?,
        elevation: read.float("elevation")?,
        tints: read.toggle("tints")?,
        color: read.vec3("color")?,
        brightens: read.toggle("brightens")?,
        intensity: read.float("intensity")?,
        shadow: read.choice("shadow")?,
    })
}
