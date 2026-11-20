use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec2, Vec3};
use rikka_renderer_3d::{
    Antialiasing, Appearance, Background, BuiltinEffect, Burst, Camera, Composite, CompositeInput,
    Curve, Draw, Effect, EffectInput, EffectInstance, EffectOutput, EffectStep, EffectUniform,
    Emission, Emitter, EmitterShape, EmitterTrack, Environment, Exposure, Forces, Frame, Glow,
    Grade, Gradient, Ground, Launch, Look, Operation, Outline, ParticleBlend, ParticleLighting,
    ParticleShape, RenderLayer, STANDARD_GRAVITY, ShaderCompiler, Tonemap, Upscale, Viewport,
    ViewportConfig,
};
use rikka_types::{Color, Rect};

use crate::support::{flat, quad};

const SIZE: u32 = 96;
const GLOW: f32 = 4.0;
const BRIGHT: f32 = 40.0;

fn renderer() -> rikka_test_renderer::TestRenderer {
    rikka_test_renderer::renderer_3d("rikka-emitter-test")
        .expect("a Vulkan 1.3 device with the mesh feature set is required for these tests")
}

fn camera() -> Camera {
    Camera::look_at(
        Vec3::new(0.0, 1.0, 4.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    )
}

fn look(emitters: Vec<Emitter>) -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.environment = Environment::None;
    look.composite = composite([], []);
    look.emitters = emitters;
    look
}

fn steady_exposure() -> Exposure {
    Exposure {
        adaptation: 0.0,
        ..Exposure::default()
    }
}

fn composite(
    scene: impl IntoIterator<Item = Operation>,
    display: impl IntoIterator<Item = Operation>,
) -> Composite {
    let mut composite = Composite::new();
    composite.push(steady_exposure());
    for operation in scene {
        composite.push(operation);
    }
    composite.push(Grade::default());
    composite.push(Tonemap::default());
    for operation in display {
        composite.push(operation);
    }
    composite
}

fn frame(look: Look, time: f32) -> Frame {
    let mut frame = Frame::new(camera());
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::splat(0.3))
        .time(time)
        .look(look)
        .outline(Outline {
            enabled: false,
            ..Outline::default()
        });
    frame
}

fn viewport(renderer: &mut Renderer, antialiasing: Antialiasing) -> Viewport {
    renderer
        .three()
        .create_viewport(
            ViewportConfig::new(SIZE, SIZE)
                .samples(1)
                .antialiasing(antialiasing),
        )
        .expect("a viewport")
}

fn render(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) -> Vec<u8> {
    renderer
        .three()
        .render(viewport, frame)
        .expect("a 3D frame draws");
    let mut target = renderer
        .create_target(SIZE, SIZE)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("a 2D frame draws");
    renderer.read_pixels(&mut target).expect("pixels come back")
}

fn shoot(renderer: &mut Renderer, frame: &Frame) -> Vec<u8> {
    let mut viewport = viewport(renderer, Antialiasing::Multisample);
    render(renderer, &mut viewport, frame)
}

fn rgb(pixels: &[u8], x: u32, y: u32) -> [f32; 3] {
    let at = ((y * SIZE + x) * 4) as usize;
    [
        pixels[at] as f32,
        pixels[at + 1] as f32,
        pixels[at + 2] as f32,
    ]
}

fn brightness(pixels: &[u8], x: u32, y: u32) -> f32 {
    let [r, g, b] = rgb(pixels, x, y);
    (r + g + b) / 3.0
}

fn centroid(pixels: &[u8]) -> Option<Vec2> {
    let mut sum = Vec2::ZERO;
    let mut weight = 0.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let value = brightness(pixels, x, y);
            if value > BRIGHT {
                sum += Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * value;
                weight += value;
            }
        }
    }
    (weight > 0.0).then(|| sum / weight)
}

fn bright_columns(pixels: &[u8]) -> Option<(u32, u32)> {
    let columns: Vec<u32> = (0..SIZE)
        .filter(|&x| (0..SIZE).any(|y| brightness(pixels, x, y) > BRIGHT))
        .collect();
    Some((*columns.first()?, *columns.last()?))
}

fn project(position: Vec3) -> Vec2 {
    let clip = camera().view_projection() * position.extend(1.0);
    let ndc = clip.truncate().truncate() / clip.w;
    (ndc * 0.5 + 0.5) * SIZE as f32
}

fn glowing(size: f32) -> Appearance {
    Appearance {
        shape: ParticleShape::Disc,
        size: Curve::constant(size),
        opacity: Curve::constant(1.0),
        emission: Curve::constant(GLOW),
        softness: 0.001,
        blend: ParticleBlend::Additive,
        ..Appearance::default()
    }
}

fn thrown(burst: f32) -> Emitter {
    Emitter {
        shape: EmitterShape::Point,
        track: EmitterTrack::fixed(Mat4::from_translation(Vec3::new(-1.0, 0.4, 0.0))),
        emission: Emission {
            rate: 0.0,
            bursts: vec![Burst::new(burst, 1)],
            lifetime: 1.2,
            ..Emission::default()
        },
        launch: Launch {
            speed: 0.0,
            velocity: Vec3::new(1.6, 3.2, 0.0),
            ..Launch::default()
        },
        forces: Forces {
            gravity: STANDARD_GRAVITY,
            drag: 0.8,
            wind: Vec3::new(0.5, 0.0, 0.0),
            ..Forces::default()
        },
        appearance: glowing(0.08),
        lighting: ParticleLighting::Unlit,
        bloom: false,
        ..Emitter::default()
    }
}

fn steady(rate: f32) -> Emitter {
    Emitter {
        shape: EmitterShape::Point,
        emission: Emission {
            rate,
            lifetime: 2.0,
            ..Emission::default()
        },
        launch: Launch {
            speed: 0.0,
            ..Launch::default()
        },
        appearance: glowing(0.06),
        lighting: ParticleLighting::Unlit,
        bloom: false,
        ..Emitter::default()
    }
}

#[test]
fn emitter_frames_depend_only_on_time() {
    let mut renderer = renderer();
    let mut sparks = Emitter::sparks(1.0, 4.0, Vec3::new(0.4, 0.0, 0.0));
    sparks.forces.turbulence.strength = 0.3;
    let snow = Emitter::snow(0.4, Vec3::new(2.0, 2.0, 2.0), Vec3::new(0.3, 0.0, 0.0));
    let petals = Emitter::petals(1.0, Vec3::new(1.5, 2.0, 1.5), Vec3::ZERO);
    let look = look(vec![sparks, snow, petals]);
    let at = 1.0;

    let first = shoot(&mut renderer, &frame(look.clone(), at));
    let again = shoot(&mut renderer, &frame(look.clone(), at));
    assert_eq!(first, again, "the same time draws the same pixels");
    assert!(
        (0..SIZE * SIZE).any(|pixel| brightness(&first, pixel % SIZE, pixel / SIZE) > BRIGHT),
        "the emitters draw something"
    );

    for path in [[0.2, at], [3.0, at]] {
        let mut viewport = viewport(&mut renderer, Antialiasing::Multisample);
        let mut last = Vec::new();
        for time in path {
            last = render(&mut renderer, &mut viewport, &frame(look.clone(), time));
        }
        assert_eq!(first, last, "time {at} reached through {path:?}");
    }
}

#[test]
fn particles_follow_the_closed_form_and_live_their_lifetime() {
    let mut renderer = renderer();
    let emitter = thrown(0.5);
    let look = look(vec![emitter.clone()]);
    for time in [0.4, 1.8] {
        assert!(emitter.particle(0, time).is_none());
        let pixels = shoot(&mut renderer, &frame(look.clone(), time));
        assert_eq!(centroid(&pixels), None, "nothing is drawn at {time}");
    }
    for time in [0.75, 1.0, 1.25] {
        let state = emitter.particle(0, time).expect("the burst particle lives");
        assert!((state.age - (time - 0.5)).abs() < 1e-5);
        let expected = project(state.position);
        let pixels = shoot(&mut renderer, &frame(look.clone(), time));
        let drawn = centroid(&pixels).expect("the particle is drawn");
        assert!(
            drawn.distance(expected) < 0.75,
            "at {time}: drawn at {drawn}, closed form at {expected}"
        );
    }
}

#[test]
fn moving_emitter_leaves_a_trail_along_its_path() {
    let mut renderer = renderer();
    let track = EmitterTrack::keyed([
        (0.0, Mat4::from_translation(Vec3::new(-1.0, 1.0, 0.0))),
        (1.0, Mat4::from_translation(Vec3::new(1.0, 1.0, 0.0))),
    ])
    .expect("a two-key track");
    let emitter = Emitter {
        track: track.clone(),
        ..steady(60.0)
    };
    let time = 1.0;
    let mut alive = 0;
    for index in 0..emitter.capacity() {
        if let Some(state) = emitter.particle(index, time) {
            alive += 1;
            let birth = track.position_at(time - state.age);
            assert!(
                state.position.distance(birth) < 1e-4,
                "particle {index} sits where the emitter was at its birth"
            );
        }
    }
    assert!((55..=61).contains(&alive), "{alive} particles live");

    let pixels = shoot(&mut renderer, &frame(look(vec![emitter]), time));
    let (left, right) = bright_columns(&pixels).expect("the trail is drawn");
    let start = project(Vec3::new(-1.0, 1.0, 0.0)).x;
    let end = project(Vec3::new(1.0, 1.0, 0.0)).x;
    assert!(
        (left as f32 - start).abs() < 3.0 && (right as f32 - end).abs() < 3.0,
        "the trail spans columns {left}..{right}, the path {start}..{end}"
    );
}

#[test]
fn drag_settles_at_terminal_velocity() {
    let wind = Vec3::new(1.5, 0.0, -0.5);
    let drag = 2.0;
    let emitter = Emitter {
        emission: Emission {
            rate: 0.0,
            bursts: vec![Burst::new(0.0, 1)],
            lifetime: 30.0,
            ..Emission::default()
        },
        launch: Launch {
            speed: 0.0,
            velocity: Vec3::new(0.0, 6.0, 3.0),
            ..Launch::default()
        },
        forces: Forces {
            gravity: STANDARD_GRAVITY,
            drag,
            wind,
            ..Forces::default()
        },
        ..steady(0.0)
    };
    let terminal = wind + STANDARD_GRAVITY / drag;
    let late = emitter.particle(0, 10.0).expect("alive");
    let later = emitter.particle(0, 11.0).expect("alive");
    assert!(
        late.velocity.distance(terminal) < 1e-3,
        "{} vs {terminal}",
        late.velocity
    );
    assert!((later.position - late.position).distance(terminal) < 1e-3);
}

#[test]
fn ground_bounces_lose_height_by_restitution_and_settle() {
    let restitution = 0.5;
    let speed = 5.0;
    let emitter = Emitter {
        emission: Emission {
            rate: 0.0,
            bursts: vec![Burst::new(0.0, 1)],
            lifetime: 10.0,
            ..Emission::default()
        },
        launch: Launch {
            speed: 0.0,
            velocity: Vec3::new(0.3, speed, 0.0),
            ..Launch::default()
        },
        forces: Forces {
            gravity: STANDARD_GRAVITY,
            ..Forces::default()
        },
        ground: Some(Ground::bounce_at(0.0, restitution, 0.0)),
        ..steady(0.0)
    };
    let mut apexes = Vec::new();
    let mut rising = true;
    let mut previous = 0.0;
    let step = 1e-3;
    for tick in 1..4000 {
        let height = emitter
            .particle(0, tick as f32 * step)
            .expect("alive")
            .position
            .y;
        assert!(height >= -1e-4, "never below the ground: {height}");
        if rising && height < previous {
            apexes.push(previous);
        }
        rising = height >= previous;
        previous = height;
    }
    let gravity = -STANDARD_GRAVITY.y;
    for (bounce, apex) in apexes.iter().take(3).enumerate() {
        let launched = speed * restitution.powi(bounce as i32);
        let expected = launched * launched / (2.0 * gravity);
        assert!(
            (apex - expected).abs() < 0.01 * expected + 1e-3,
            "bounce {bounce}: apex {apex}, expected {expected}"
        );
    }
    let rest = emitter.particle(0, 9.0).expect("resting");
    assert_eq!(rest.position.y, 0.0);
    assert_eq!(rest.velocity, Vec3::ZERO);
}

#[test]
fn bursts_and_streams_keep_particles_within_their_lifetimes() {
    let burst = Emitter {
        emission: Emission {
            rate: 0.0,
            bursts: vec![Burst::new(1.0, 12), Burst::new(2.0, 4).over(0.5)],
            lifetime: 0.75,
            ..Emission::default()
        },
        ..steady(0.0)
    };
    let alive = |emitter: &Emitter, time: f32| {
        (0..emitter.capacity())
            .filter(|&i| emitter.particle(i, time).is_some())
            .count()
    };
    for (time, expected) in [
        (0.99, 0),
        (1.01, 12),
        (1.8, 0),
        (2.24, 2),
        (2.6, 4),
        (3.3, 0),
    ] {
        assert_eq!(alive(&burst, time), expected, "at {time}");
    }

    let stream = Emitter {
        emission: Emission {
            rate: 50.0,
            lifetime: 1.5,
            lifetime_variance: 0.4,
            start: 0.5,
            end: 4.0,
            ..Emission::default()
        },
        ..steady(0.0)
    };
    assert_eq!(alive(&stream, 0.49), 0);
    for time in [1.0, 2.7, 3.9, 4.5] {
        for index in 0..stream.capacity() {
            if let Some(state) = stream.particle(index, time) {
                assert!(state.age >= 0.0 && state.age <= state.lifetime);
                assert!(state.lifetime <= 1.5 && state.lifetime >= 1.5 * 0.6 - 1e-5);
                assert!(time - state.age >= 0.5 && time - state.age < 4.0);
            }
        }
    }
    let steady_count = alive(&stream, 3.0) as f32;
    let expected = 50.0 * 1.5 * 0.8;
    assert!(
        (steady_count - expected).abs() < 10.0,
        "{steady_count} alive, about {expected} expected"
    );
    assert_eq!(alive(&stream, 5.6), 0);
}

fn wall_scene(renderer: &mut Renderer, look: Look) -> Frame {
    let wall = quad(renderer, 0.0, 3.0);
    let grey = flat(renderer, [0.5, 0.5, 0.5, 1.0]);
    let mut frame = frame(look, 1.0);
    frame.draw(Draw::new(wall, grey).transform(Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0))));
    frame
}

fn resting(appearance: Appearance, z: f32) -> Emitter {
    Emitter {
        track: EmitterTrack::fixed(Mat4::from_translation(Vec3::new(0.0, 1.0, z))),
        emission: Emission {
            rate: 0.0,
            bursts: vec![Burst::new(0.0, 1)],
            lifetime: 5.0,
            ..Emission::default()
        },
        launch: Launch {
            speed: 0.0,
            ..Launch::default()
        },
        appearance,
        lighting: ParticleLighting::Unlit,
        bloom: false,
        ..Emitter::default()
    }
}

#[test]
fn soft_particles_fade_near_geometry() {
    let mut renderer = renderer();
    let centre = SIZE / 2;
    let shot = |renderer: &mut Renderer, softness: f32| {
        let appearance = Appearance {
            softness,
            emission: Curve::constant(0.4),
            ..glowing(0.6)
        };
        let frame = wall_scene(renderer, look(vec![resting(appearance, 0.05)]));
        brightness(&shoot(renderer, &frame), centre, centre)
    };
    let crisp = shot(&mut renderer, 0.001);
    let soft = shot(&mut renderer, 1.0);
    let empty = wall_scene(&mut renderer, look(Vec::new()));
    let bare = brightness(&shoot(&mut renderer, &empty), centre, centre);
    assert!(
        crisp > bare + 60.0,
        "a crisp particle covers the wall: {crisp} vs {bare}"
    );
    assert!(
        soft < bare + 0.25 * (crisp - bare),
        "a soft particle fades into the wall: {soft} (bare {bare}, crisp {crisp})"
    );
}

#[test]
fn additive_and_alpha_particles_blend_differently() {
    let mut renderer = renderer();
    let centre = SIZE / 2;
    let dark = |blend| Appearance {
        color: Gradient::constant(Vec3::ZERO),
        emission: Curve::constant(0.0),
        blend,
        ..glowing(0.6)
    };
    let mut shot = |look: Look| {
        let frame = wall_scene(&mut renderer, look);
        brightness(&shoot(&mut renderer, &frame), centre, centre)
    };
    let bare = shot(look(Vec::new()));
    let additive = shot(look(vec![resting(dark(ParticleBlend::Additive), 0.5)]));
    let alpha = shot(look(vec![resting(dark(ParticleBlend::Alpha), 0.5)]));
    assert!(bare > 30.0, "the wall is lit: {bare}");
    assert!(
        (additive - bare).abs() <= 1.0,
        "adding black leaves the wall: {additive} vs {bare}"
    );
    assert!(
        alpha < 0.1 * bare,
        "black alpha covers the wall: {alpha} vs {bare}"
    );
}

#[test]
fn layer_tint_isolates_emitter_layers() {
    let mut renderer = renderer();
    let tint = renderer
        .three()
        .create_effect(&BuiltinEffect::LayerTint.effect())
        .expect("built-in effects are valid");
    let centre = SIZE / 2;
    let mut shot = |layers: u32| {
        let mut look = look(vec![Emitter {
            layers,
            ..resting(
                Appearance {
                    blend: ParticleBlend::Alpha,
                    ..glowing(0.6)
                },
                0.5,
            )
        }]);
        let tinted = EffectInstance::new(tint)
            .set("mask", 2u32)
            .set("amount", 1.0)
            .set("color", [1.0, 0.0, 0.0])
            .set("farthest", 0.0);
        look.composite = composite([tinted.into()], []);
        let frame = wall_scene(&mut renderer, look);
        let pixels = shoot(&mut renderer, &frame);
        rgb(&pixels, centre, centre)
    };
    let [r, g, b] = shot(2);
    assert!(
        r > 120.0 && g < 40.0 && b < 40.0,
        "layer 2 is tinted: {r} {g} {b}"
    );
    let [r, g, b] = shot(1);
    assert!(g > 120.0 && b > 120.0, "layer 1 is left alone: {r} {g} {b}");
}

#[test]
fn emission_reaches_the_emission_layer() {
    let mut renderer = renderer();
    let mut shot = |writes: bool| {
        let mut look = look(vec![Emitter {
            bloom: writes,
            ..resting(glowing(0.2), 0.5)
        }]);
        let mut glowed = Composite::new();
        let exposed = glowed.push(steady_exposure());
        glowed.node(
            Glow {
                bloom: 1.0,
                diffusion: 0.0,
                streaks: 0.0,
                flare: 0.0,
                ..Glow::default()
            },
            [
                (Composite::IMAGE, CompositeInput::Node(exposed)),
                (Glow::SOURCE, CompositeInput::Layer(RenderLayer::Emission)),
            ],
        );
        glowed.push(Grade::default());
        glowed.push(Tonemap::default());
        look.composite = glowed;
        let pixels = shoot(&mut renderer, &frame(look, 1.0));
        (0..SIZE * SIZE)
            .map(|pixel| brightness(&pixels, pixel % SIZE, pixel / SIZE))
            .sum::<f32>()
    };
    let glowing = shot(true);
    let dull = shot(false);
    assert!(
        glowing > dull + 100.0,
        "emissive particles bloom: {glowing} vs {dull}"
    );
}

const VELOCITY_PROBE: &str = r#"
import rikka;

struct Parameters {
    [Default(20.0)] float gain;
};

struct Push {
    Parameters* parameters;
    uint velocity;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void probe(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float2 velocity = rikka_effect_load(push.velocity, pixel).xy;
    rikka_effect_store(push.target, pixel, float4(saturate(abs(velocity) * push.parameters.gain), 0.0, 1.0));
}
"#;

const PROBE_GAIN: f32 = 20.0;

fn velocity_probe(renderer: &mut Renderer) -> EffectInstance {
    let code = ShaderCompiler::new()
        .expect("libslang loads")
        .effect_code("velocity", VELOCITY_PROBE, "probe")
        .expect("the probe compiles");
    let effect = Effect::new("velocity").input(Composite::IMAGE).step(
        EffectStep::new(code)
            .read("velocity", EffectInput::Velocity)
            .write("target", EffectOutput::Image)
            .uniform("size", EffectUniform::Size),
    );
    let id = renderer
        .three()
        .create_effect(&effect)
        .expect("the probe is a valid effect");
    EffectInstance::new(id).set("gain", PROBE_GAIN)
}

#[test]
fn moving_particles_carry_motion_and_leave_no_ghost_under_temporal_antialiasing() {
    let mut renderer = renderer();
    let emitter = Emitter {
        track: EmitterTrack::fixed(Mat4::from_translation(Vec3::new(-1.6, 1.0, 0.0))),
        emission: Emission {
            rate: 0.0,
            bursts: vec![Burst::new(0.0, 1)],
            lifetime: 10.0,
            ..Emission::default()
        },
        launch: Launch {
            speed: 0.0,
            velocity: Vec3::new(3.0, 0.0, 0.0),
            ..Launch::default()
        },
        appearance: glowing(0.2),
        ..steady(0.0)
    };
    let step = 1.0 / 60.0;
    let frames = 60;
    let now = frames as f32 * step;
    let position = |time: f32| project(emitter.particle(0, time).expect("alive").position);
    let here = position(now);
    let run = |renderer: &mut Renderer, antialiasing: Antialiasing, look: &Look| {
        let mut viewport = viewport(renderer, antialiasing);
        let mut last = Vec::new();
        for at in 0..=frames {
            last = render(
                renderer,
                &mut viewport,
                &frame(look.clone(), at as f32 * step),
            );
        }
        last
    };

    let last = run(
        &mut renderer,
        Antialiasing::Temporal,
        &look(vec![emitter.clone()]),
    );
    let drawn = centroid(&last).expect("the particle is drawn");
    assert!(
        drawn.distance(here) < 1.0,
        "the particle is drawn where it is: {drawn} vs {here}"
    );
    let row = here.y as u32;
    for behind in [6, 12, 24] {
        let before = position(now - behind as f32 * step);
        let ghost = brightness(&last, before.x as u32, row);
        assert!(
            ghost < 12.0,
            "{behind} frames behind at column {}: {ghost}",
            before.x
        );
    }

    let mut probed = look(vec![emitter.clone()]);
    probed.composite = composite([], [velocity_probe(&mut renderer).into()]);
    let motion = run(
        &mut renderer,
        Antialiasing::Upscale(Upscale::fixed(1.0)),
        &probed,
    );
    let shift = (here - position(now - step)) / SIZE as f32;
    let expected = (shift.abs() * PROBE_GAIN).min(Vec2::ONE) * 255.0;
    let [x, y, _] = rgb(&motion, here.x as u32, here.y as u32);
    assert!(
        (x - expected.x).abs() < 16.0 && (y - expected.y).abs() < 16.0,
        "particle motion {x} {y}, expected {expected}"
    );
    let [still, _, _] = rgb(&motion, here.x as u32, here.y as u32 - 12);
    assert!(still < 4.0, "the empty sky beside it is still: {still}");
}
