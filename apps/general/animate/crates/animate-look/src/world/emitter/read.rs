use rikka_renderer_3d::{Attractor, Ground, Launch, Stretch, Turbulence, Twinkle};
use rikka_shader_graph::Record;

use crate::error::Error;
use crate::world::emitter::nodes::{
    AREA, EMITTER_NODE, MAX_BURST_REPEATS, PITCH, POSITION, RADIUS, ROLL, SCALE, SPEED, YAW,
    preset_spec,
};
use crate::world::emitter::{
    AppearanceLook, BurstLook, CustomEmitter, EmissionLook, EmitterLook, EmitterSource, ForcesLook,
    GroundLook, Placement, PresetEmitter, ShapeLook, Spread, SpriteLook, Timing,
};
use crate::world::nodes::EMITTERS;
use crate::world::read::Read;

pub(crate) fn emitter(record: &Record) -> Result<EmitterLook, Error> {
    let read = Read::new(record);
    let scaled = record.id == EMITTER_NODE;
    let source = match preset_spec(&record.id) {
        Some(spec) => EmitterSource::Preset(preset(&read, spec.preset, spec.spread)?),
        None if scaled => EmitterSource::Custom(Box::new(custom(&read)?)),
        None => {
            return Err(Error::Record {
                node: record.id.clone(),
                port: EMITTERS.to_owned(),
                expected: "a particle emitter",
            });
        }
    };
    Ok(EmitterLook {
        node: record.path.clone(),
        source,
        placement: placement(&read, scaled)?,
        track: Vec::new(),
    })
}

pub(crate) fn placement(read: &Read<'_>, scaled: bool) -> Result<Placement, Error> {
    Ok(Placement {
        position: read.vec3(POSITION)?,
        yaw: read.float(YAW)?,
        pitch: read.float(PITCH)?,
        roll: read.float(ROLL)?,
        scale: match scaled {
            true => read.vec3(SCALE)?,
            false => Placement::default().scale,
        },
    })
}

fn timing(read: &Read<'_>) -> Result<Timing, Error> {
    Ok(Timing {
        start: read.float("start")?,
        end: read.float("end")?,
    })
}

fn preset(
    read: &Read<'_>,
    preset: crate::world::emitter::EmitterPreset,
    spread: Spread,
) -> Result<PresetEmitter, Error> {
    Ok(PresetEmitter {
        preset,
        amount: read.float("amount")?,
        spread: match spread {
            Spread::Area(_) => Spread::Area(read.vec3(AREA)?),
            Spread::Radius(_) => Spread::Radius(read.float(RADIUS)?),
            Spread::Speed(_) => Spread::Speed(read.float(SPEED)?),
        },
        wind: read.vec3("wind")?,
        tint: read.vec3("tint")?,
        timing: timing(read)?,
        seed: read.whole("seed")?,
    })
}

fn custom(read: &Read<'_>) -> Result<CustomEmitter, Error> {
    Ok(CustomEmitter {
        seed: read.whole("seed")?,
        shape: ShapeLook {
            kind: read.choice("shape")?,
            radius: read.float(RADIUS)?,
            extent: read.vec3("extent")?,
            angle: read.float("angle")?,
            length: read.float("length")?,
        },
        emission: EmissionLook {
            rate: read.float("rate")?,
            timing: timing(read)?,
            lifetime: read.float("lifetime")?,
            lifetime_variance: read.float("lifetime_variance")?,
            max_particles: read.whole("max_particles")?,
            burst: BurstLook {
                count: read.whole("burst_count")?,
                time: read.float("burst_time")?,
                spread: read.float("burst_spread")?,
                repeats: read.whole("burst_repeats")?.min(MAX_BURST_REPEATS),
                interval: read.float("burst_interval")?,
            },
        },
        launch: Launch {
            speed: read.float(SPEED)?,
            speed_variance: read.float("speed_variance")?,
            velocity: read.vec3("velocity")?,
            inherit: read.float("inherit")?,
        },
        forces: ForcesLook {
            gravity: read.vec3("gravity")?,
            drag: read.float("drag")?,
            drag_variance: read.float("drag_variance")?,
            wind: read.vec3("wind")?,
            turbulence: Turbulence {
                strength: read.float("turbulence")?,
                frequency: read.float("turbulence_frequency")?,
                flow: read.float("turbulence_flow")?,
            },
            attracts: read.toggle("attractor")?,
            attractor: Attractor {
                position: read.vec3("attractor_position")?,
                strength: read.float("attractor_strength")?,
                rate: read.float("attractor_rate")?,
            },
        },
        ground: GroundLook {
            enabled: read.toggle("ground")?,
            ground: Ground {
                height: read.float("ground_height")?,
                contact: read.choice("contact")?,
                restitution: read.float("restitution")?,
                friction: read.float("friction")?,
                fade: read.float("ground_fade")?,
            },
        },
        appearance: AppearanceLook {
            particle: read.choice("particle")?,
            size: read.float("size")?,
            size_end: read.float("size_end")?,
            size_variance: read.float("size_variance")?,
            aspect: read.float("aspect")?,
            colors: [
                read.vec3("color")?,
                read.vec3("color_early")?,
                read.vec3("color_late")?,
                read.vec3("color_death")?,
            ],
            early: read.float("early")?,
            late: read.float("late")?,
            coloring: read.choice("coloring")?,
            opacity: read.float("opacity")?,
            fade_in: read.float("fade_in")?,
            fade_out: read.float("fade_out")?,
            emission: read.float("emission")?,
            emission_end: read.float("emission_end")?,
            twinkle: Twinkle {
                depth: read.float("twinkle")?,
                rate: read.float("twinkle_rate")?,
            },
            rotation: read.float("rotation")?,
            spin: read.float("spin")?,
            spin_variance: read.float("spin_variance")?,
            stretch: Stretch {
                seconds: read.float("stretch")?,
                min: read.float("stretch_min")?,
                max: read.float("stretch_max")?,
            },
            facing: read.choice("facing")?,
            facing_axis: read.vec3("facing_axis")?,
            softness: read.float("softness")?,
            blend: read.choice("blend")?,
        },
        sprite: SpriteLook {
            texture: read.asset("sprite")?,
            columns: read.whole("columns")?,
            rows: read.whole("rows")?,
            frame_rate: read.float("frame_rate")?,
            random_start: read.toggle("random_start")?,
        },
        lit: read.toggle("lit")?,
        translucency: read.float("translucency")?,
        layers: read.whole("layers")?,
        bloom: read.toggle("bloom")?,
    })
}
