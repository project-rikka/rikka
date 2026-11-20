use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Coloring, Emitter, GroundContact, NITS_PER_RADIANCE_UNIT, ParticleBlend, TextureKind,
};
use rikka_shader_graph::{Category, Hint, NodeSpec, PortSpec, ValueType};

use crate::world::choice::options;
use crate::world::emitter::{EmitterPreset, Facing, ShapeKind, Spread};
use crate::world::nodes::{angle, color, ranged, record, soft, vector, whole};

pub(crate) const EMITTER: &str = "emitter";
pub(crate) const EMITTER_NODE: &str = "world.emitter";

pub const POSITION: &str = "position";
pub(crate) const YAW: &str = "yaw";
pub(crate) const PITCH: &str = "pitch";
pub(crate) const ROLL: &str = "roll";
pub(crate) const SCALE: &str = "scale";
pub(crate) const PLACEMENT: [&str; 5] = [POSITION, YAW, PITCH, ROLL, SCALE];

pub(crate) const AREA: &str = "area";
pub(crate) const RADIUS: &str = "radius";
pub(crate) const SPEED: &str = "speed";

pub(crate) const MAX_BURST_REPEATS: u32 = 15;

pub(crate) struct PresetSpec {
    pub(crate) preset: EmitterPreset,
    pub(crate) id: &'static str,
    title: &'static str,
    doc: &'static str,
    pub(crate) spread: Spread,
}

macro_rules! preset_doc {
    ($lead:literal) => {
        concat!(
            $lead,
            " Placed at position and turned by yaw, pitch and roll; amount scales how many, wind drifts them, tint multiplies their colours and seed varies the pattern. They are born from start until end (an end at or before start never ends) where the emitter is at that moment, so driven or keyed placement leaves trails."
        )
    };
}

pub(crate) const PRESETS: [PresetSpec; 10] = [
    PresetSpec {
        preset: EmitterPreset::Rain,
        id: "world.emitter.rain",
        title: "Rain",
        doc: preset_doc!(
            "Rain streaking down from a slab area.y metres above position, area.x by area.z metres to either side, dying on the ground at height 0."
        ),
        spread: Spread::Area(Vec3::new(5.0, 5.0, 5.0)),
    },
    PresetSpec {
        preset: EmitterPreset::Snow,
        id: "world.emitter.snow",
        title: "Snow",
        doc: preset_doc!(
            "Snow drifting down from a slab area.y metres above position, area.x by area.z metres to either side, settling on the ground at height 0 and melting away."
        ),
        spread: Spread::Area(Vec3::new(5.0, 5.0, 5.0)),
    },
    PresetSpec {
        preset: EmitterPreset::Sparks,
        id: "world.emitter.sparks",
        title: "Sparks",
        doc: preset_doc!(
            "Hot glowing sparks shot up a narrow cone at speed metres per second, falling, bouncing on the ground at height 0 and cooling."
        ),
        spread: Spread::Speed(4.0),
    },
    PresetSpec {
        preset: EmitterPreset::Embers,
        id: "world.emitter.embers",
        title: "Embers",
        doc: preset_doc!(
            "Glowing embers rising from a disc of radius metres, swirling and dimming."
        ),
        spread: Spread::Radius(0.5),
    },
    PresetSpec {
        preset: EmitterPreset::Petals,
        id: "world.emitter.petals",
        title: "Petals",
        doc: preset_doc!(
            "Blossom petals tumbling down from a slab area.y metres above position, area.x by area.z metres to either side, settling on the ground at height 0."
        ),
        spread: Spread::Area(Vec3::new(4.0, 4.0, 4.0)),
    },
    PresetSpec {
        preset: EmitterPreset::Fireflies,
        id: "world.emitter.fireflies",
        title: "Fireflies",
        doc: preset_doc!(
            "Twinkling fireflies wandering in a box centred area.y metres above position with half sizes area."
        ),
        spread: Spread::Area(Vec3::new(4.0, 1.0, 4.0)),
    },
    PresetSpec {
        preset: EmitterPreset::Dust,
        id: "world.emitter.dust",
        title: "Dust",
        doc: preset_doc!(
            "Soft dust puffs blown out of a wide cone of radius metres, growing as they fade."
        ),
        spread: Spread::Radius(0.5),
    },
    PresetSpec {
        preset: EmitterPreset::Magic,
        id: "world.emitter.magic",
        title: "Magic",
        doc: preset_doc!("Twinkling star sparkles rising from a sphere of radius metres."),
        spread: Spread::Radius(0.5),
    },
    PresetSpec {
        preset: EmitterPreset::Confetti,
        id: "world.emitter.confetti",
        title: "Confetti",
        doc: preset_doc!(
            "Coloured confetti fluttering down from a slab area.y metres above position, area.x by area.z metres to either side, settling on the ground at height 0."
        ),
        spread: Spread::Area(Vec3::new(3.0, 4.0, 3.0)),
    },
    PresetSpec {
        preset: EmitterPreset::Leaves,
        id: "world.emitter.leaves",
        title: "Leaves",
        doc: preset_doc!(
            "Autumn leaves tumbling down from a slab area.y metres above position, area.x by area.z metres to either side, settling on the ground at height 0."
        ),
        spread: Spread::Area(Vec3::new(4.0, 4.0, 4.0)),
    },
];

pub(crate) fn preset_spec(id: &str) -> Option<&'static PresetSpec> {
    PRESETS.iter().find(|spec| spec.id == id)
}

fn placement(scaled: bool) -> Vec<PortSpec> {
    let mut ports = vec![
        vector(POSITION, Vec3::ZERO),
        angle(YAW, 0.0),
        angle(PITCH, 0.0),
        angle(ROLL, 0.0),
    ];
    if scaled {
        ports.push(vector(SCALE, Vec3::ONE));
    }
    ports
}

fn timing() -> [PortSpec; 2] {
    [soft("start", 0.0, 0.0, 60.0), soft("end", 0.0, 0.0, 60.0)]
}

fn preset_node(spec: &PresetSpec) -> NodeSpec {
    let mut inputs = placement(false);
    inputs.push(soft("amount", 1.0, 0.0, 10.0));
    inputs.push(match spec.spread {
        Spread::Area(area) => vector(AREA, area),
        Spread::Radius(radius) => soft(RADIUS, radius, 0.01, 5.0),
        Spread::Speed(speed) => soft(SPEED, speed, 0.0, 20.0),
    });
    inputs.push(vector("wind", Vec3::ZERO));
    inputs.push(color("tint", Vec3::ONE));
    inputs.extend(timing());
    inputs.push(whole("seed", 0));
    record(
        spec.id,
        spec.title,
        Category::Environment,
        spec.doc,
        EMITTER,
        inputs,
    )
}

fn custom_node() -> NodeSpec {
    let base = Emitter::default();
    let emission = &base.emission;
    let launch = base.launch;
    let forces = base.forces;
    let look = base.appearance;
    let ground = rikka_renderer_3d::Ground::default();
    let mut inputs = placement(true);
    inputs.extend([
        whole("seed", base.seed),
        options("shape", ShapeKind::Point),
        soft(RADIUS, 0.5, 0.0, 5.0),
        vector("extent", Vec3::splat(0.5)),
        angle("angle", 0.5),
        soft("length", 1.0, 0.0, 10.0),
        soft("rate", emission.rate, 0.0, 1000.0),
    ]);
    inputs.extend(timing());
    inputs.extend([
        soft("lifetime", emission.lifetime, 0.1, 20.0),
        ranged("lifetime_variance", emission.lifetime_variance, 0.0, 1.0),
        whole("max_particles", emission.max_particles),
        whole("burst_count", 0),
        soft("burst_time", 0.0, 0.0, 60.0),
        soft("burst_spread", 0.0, 0.0, 5.0),
        whole("burst_repeats", 0),
        soft("burst_interval", 1.0, 0.0, 10.0),
        soft(SPEED, launch.speed, 0.0, 20.0),
        ranged("speed_variance", launch.speed_variance, 0.0, 1.0),
        vector("velocity", launch.velocity),
        ranged("inherit", launch.inherit, 0.0, 1.0),
        vector("gravity", forces.gravity),
        soft("drag", forces.drag, 0.0, 10.0),
        ranged("drag_variance", forces.drag_variance, 0.0, 1.0),
        vector("wind", forces.wind),
        soft("turbulence", forces.turbulence.strength, 0.0, 2.0),
        soft("turbulence_frequency", 1.0, 0.0, 5.0),
        soft("turbulence_flow", 0.5, 0.0, 2.0),
        PortSpec::toggle("attractor", false),
        vector("attractor_position", Vec3::new(0.0, 1.0, 0.0)),
        ranged("attractor_strength", 1.0, 0.0, 1.0),
        soft("attractor_rate", 1.0, 0.0, 10.0),
        PortSpec::toggle("ground", false),
        soft("ground_height", ground.height, -10.0, 10.0),
        options("contact", GroundContact::Bounce),
        ranged("restitution", ground.restitution, 0.0, 1.0),
        ranged("friction", ground.friction, 0.0, 1.0),
        soft("ground_fade", ground.fade, 0.0, 10.0),
        options("particle", look.shape),
        soft("size", look.size.sample(0.0), 0.001, 1.0),
        soft("size_end", 1.0, 0.0, 4.0),
        ranged("size_variance", look.size_variance, 0.0, 1.0),
        soft("aspect", look.aspect, 0.1, 4.0),
        color("color", Vec3::ONE),
        color("color_early", Vec3::ONE),
        color("color_late", Vec3::ONE),
        color("color_death", Vec3::ONE),
        ranged("early", 1.0 / 3.0, 0.0, 1.0),
        ranged("late", 2.0 / 3.0, 0.0, 1.0),
        options("coloring", Coloring::OverAge),
        ranged("opacity", 1.0, 0.0, 1.0),
        ranged("fade_in", 0.1, 0.0, 1.0),
        ranged("fade_out", 0.2, 0.0, 1.0),
        soft(
            "emission",
            look.emission.sample(0.0) * NITS_PER_RADIANCE_UNIT,
            0.0,
            500_000.0,
        ),
        soft("emission_end", 1.0, 0.0, 4.0),
        ranged("twinkle", look.twinkle.depth, 0.0, 1.0),
        soft("twinkle_rate", 1.0, 0.0, 10.0),
        angle("rotation", look.rotation),
        soft("spin", look.spin, -10.0, 10.0),
        ranged("spin_variance", look.spin_variance, 0.0, 1.0),
        soft("stretch", look.stretch.seconds, 0.0, 0.2),
        soft("stretch_min", look.stretch.min, 0.0, 1.0),
        soft("stretch_max", 0.5, 0.0, 2.0),
        options("facing", Facing::Camera),
        vector("facing_axis", Vec3::Y),
        ranged("softness", look.softness, 0.0, 1.0),
        options("blend", ParticleBlend::Alpha),
        PortSpec::empty("sprite", ValueType::Texture).hint(Hint::Texture(TextureKind::Colour)),
        whole("columns", 1),
        whole("rows", 1),
        soft("frame_rate", 0.0, 0.0, 60.0),
        PortSpec::toggle("random_start", false),
        PortSpec::toggle("lit", true),
        ranged("translucency", 0.3, 0.0, 1.0),
        whole("layers", base.layers),
        PortSpec::toggle("bloom", base.bloom),
    ]);
    record(
        EMITTER_NODE,
        "Particle emitter",
        Category::Environment,
        "A particle emitter described in full. Particles are born on its shape (a point; a sphere, disc or cone of radius, the cone opening by angle; a box of half sizes extent; a line of length along x) at rate per second, plus burst_count at once at burst_time, spread over burst_spread seconds and repeated burst_repeats more times every burst_interval, from start until end (an end at or before start never ends). Each lives lifetime seconds (lifetime_variance shortens some), leaves at speed along the shape's direction plus velocity, keeps inherit of the emitter's own motion, falls by gravity, drags toward wind, swirls by turbulence, is pulled to attractor_position when attractor is on and meets the ground at ground_height when ground is on, bouncing or dying by contact. Over its life size grows to size_end times, opacity fades in and out by the fade shares, colour runs through four colours (early and late are where the middle two sit) or picks one per particle, and emission (nits) goes to emission_end times. sprite draws a picture cut into columns by rows frames played at frame_rate. Placed at position, turned by yaw, pitch and roll and scaled by scale; particles are born where the emitter is at that moment, so driven or keyed placement leaves trails.",
        EMITTER,
        inputs,
    )
}

#[must_use]
pub(crate) fn emitter_nodes() -> Vec<NodeSpec> {
    let mut nodes = vec![custom_node()];
    nodes.extend(PRESETS.iter().map(preset_node));
    nodes
}
