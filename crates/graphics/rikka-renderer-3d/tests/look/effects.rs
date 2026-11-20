use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Error;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3};
use rikka_renderer_3d::{
    Antialiasing, Background, Bounds, BuiltinEffect, Camera, Composite, Draw, DynamicScale, Effect,
    EffectId, EffectInput, EffectInstance, EffectOutput, EffectStage, EffectStep, Environment,
    Exposure, Frame, Grade, Light, Literal, Look, MaterialId, MeshId, ParameterKind, RenderScale,
    ShaderCompiler, ShadowQuality, Tonemap, Upscale, Viewport, ViewportConfig,
};
use rikka_types::{Color, Rect};

use crate::support::{SIZE, ambient_frame, flat, quad, renderer, viewport};

const PROBE: &str = r#"
import rikka;

struct Parameters {
    [Default(0.0)] uint mode;
    [Default(1.0)] float gain;
    float4 spare0;
    float4 spare1;
    float4 spare2;
    float4 spare3;
    float4 spare4;
    float4 spare5;
    float4 spare6;
    float4 spare7;
    float4 spare8;
    [Default(0.25, 0.5, 0.75, 1.0)] float4 marker;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint normals;
    uint velocity;
    uint history;
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
    Parameters parameters = *push.parameters;
    float4 scene = rikka_effect_load(push.source, pixel);
    float4 written = scene;
    if (parameters.mode == 1u) {
        float4 normal = rikka_effect_load(push.normals, pixel);
        written = float4(normal.xyz * 0.5 + 0.5, 1.0) * normal.w;
    } else if (parameters.mode == 2u) {
        float4 velocity = rikka_effect_load(push.velocity, pixel);
        written = float4(saturate(abs(velocity.xy) * parameters.gain), 0.0, 1.0);
    } else if (parameters.mode == 3u) {
        written = lerp(scene, rikka_effect_load(push.history, pixel), 0.5);
    } else if (parameters.mode == 4u) {
        written = float4(parameters.marker.rgb, 1.0);
    }
    rikka_effect_store(push.target, pixel, written);
}
"#;

struct Wall {
    near: MeshId,
    far: MeshId,
    grey: MaterialId,
}

fn wall(renderer: &mut Renderer) -> Wall {
    Wall {
        near: quad(renderer, 1.0, 0.4),
        far: quad(renderer, 0.0, 4.0),
        grey: flat(renderer, [0.6, 0.6, 0.6, 1.0]),
    }
}

fn image() -> EffectInput {
    EffectInput::Input(Composite::IMAGE.to_owned())
}

fn compiler() -> ShaderCompiler {
    ShaderCompiler::new().expect("libslang loads")
}

fn look(effects: Vec<EffectInstance>) -> Look {
    placed(Vec::new(), effects)
}

fn placed(scene: Vec<EffectInstance>, display: Vec<EffectInstance>) -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.environment = Environment::None;
    let mut composite = Composite::new();
    composite.push(Exposure {
        adaptation: 0.0,
        ..Exposure::default()
    });
    for instance in scene {
        composite.push(instance);
    }
    composite.push(Grade::default());
    composite.push(Tonemap::default());
    for instance in display {
        composite.push(instance);
    }
    look.composite = composite;
    look
}

fn frame(scene: &Wall, look: Look, near_layers: u32) -> Frame {
    let mut frame = ambient_frame(0.0);
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::splat(0.4))
        .look(look);
    frame.light(
        Light::directional(-Vec3::Z)
            .intensity(2.0)
            .shadow(ShadowQuality::None),
    );
    frame.draw(Draw::new(scene.far, scene.grey).layers(1));
    frame.draw(
        Draw::new(scene.near, scene.grey)
            .transform(Mat4::from_translation(Vec3::new(-0.5, 0.0, 0.0)))
            .layers(near_layers),
    );
    frame
}

fn draw(renderer: &mut Renderer, viewport: &mut Viewport, frame: &Frame) -> Result<Vec<u8>, Error> {
    renderer.three().render(viewport, frame)?;
    let mut target = renderer.create_target(SIZE, SIZE)?;
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
        viewport.texture(),
    ));
    renderer.draw(&mut scene, &mut target)?;
    Ok(renderer.read_pixels(&mut target)?)
}

fn render(renderer: &mut Renderer, samples: u32, frame: &Frame) -> Result<Vec<u8>, Error> {
    let mut viewport = viewport(renderer, samples);
    draw(renderer, &mut viewport, frame)
}

fn rgb(pixels: &[u8], x: u32, y: u32) -> [i32; 3] {
    let at = ((y * SIZE + x) * 4) as usize;
    [0, 1, 2].map(|channel| i32::from(pixels[at + channel]))
}

fn luma(pixels: &[u8], x: u32, y: u32) -> i32 {
    let [r, g, b] = rgb(pixels, x, y);
    (2126 * r + 7152 * g + 722 * b) / 10000
}

fn create(renderer: &mut Renderer, effect: &Effect) -> EffectId {
    renderer
        .three()
        .create_effect(effect)
        .expect("the effect is valid")
}

fn probe(compiler: &ShaderCompiler) -> Effect {
    let code = compiler
        .effect_code("probe", PROBE, "probe")
        .expect("the probe compiles");
    Effect::new("probe").input(Composite::IMAGE).step(
        EffectStep::new(code)
            .read("source", image())
            .read("normals", EffectInput::Normals)
            .read("velocity", EffectInput::Velocity)
            .read("history", EffectInput::History)
            .write("target", EffectOutput::Image)
            .uniform("size", rikka_renderer_3d::EffectUniform::Size),
    )
}

#[test]
fn vignette_darkens_corners_before_and_after_the_tonemap() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let plain = render(&mut renderer, 4, &frame(&scene, look(Vec::new()), 1)).expect("frame draws");
    let corner = (4, SIZE - 4);
    let centre = (SIZE / 2 + 20, SIZE / 2);
    let vignette = create(&mut renderer, &BuiltinEffect::Vignette.effect());
    let darkened = || vec![EffectInstance::new(vignette).set("strength", 1.0)];
    for (insertion, look) in [
        ("before", placed(darkened(), Vec::new())),
        ("after", placed(Vec::new(), darkened())),
    ] {
        let shaded = render(&mut renderer, 4, &frame(&scene, look, 1)).expect("frame draws");
        let plain_corner = luma(&plain, corner.0, corner.1);
        let dark_corner = luma(&shaded, corner.0, corner.1);
        assert!(
            dark_corner + 20 < plain_corner,
            "{insertion:?}: corner {dark_corner} vs {plain_corner}"
        );
        let plain_centre = luma(&plain, centre.0, centre.1);
        let kept_centre = luma(&shaded, centre.0, centre.1);
        assert!(
            (kept_centre - plain_centre).abs() <= 3,
            "{insertion:?}: centre {kept_centre} vs {plain_centre}"
        );
    }
}

#[test]
fn film_grain_scatters_flat_colour() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let grain = create(&mut renderer, &BuiltinEffect::FilmGrain.effect());
    let spread = |pixels: &[u8]| {
        let values: Vec<i32> = (0..32)
            .map(|step| luma(pixels, SIZE / 2 + 20 + step % 8, SIZE / 2 + step / 8))
            .collect();
        values.iter().max().unwrap() - values.iter().min().unwrap()
    };
    let plain = render(&mut renderer, 1, &frame(&scene, look(Vec::new()), 1)).expect("frame draws");
    let effects = vec![EffectInstance::new(grain).set("amount", 0.3)];
    let grainy = render(&mut renderer, 1, &frame(&scene, look(effects), 1)).expect("frame draws");
    assert!(
        spread(&plain) <= 2,
        "flat wall varies by {}",
        spread(&plain)
    );
    assert!(
        spread(&grainy) >= 10,
        "grain varies by only {}",
        spread(&grainy)
    );
}

#[test]
fn layer_tint_follows_layers_and_depth() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let tint = create(&mut renderer, &BuiltinEffect::LayerTint.effect());
    let near = (SIZE / 2 - 18, SIZE / 2);
    let far = (SIZE / 2 + 30, SIZE / 2);
    let red = |pixels: &[u8], (x, y): (u32, u32)| {
        let [r, g, b] = rgb(pixels, x, y);
        r > 120 && g < 40 && b < 40
    };
    for antialiasing in [
        Antialiasing::Multisample,
        Antialiasing::Upscale(Upscale::fixed(0.5)),
    ] {
        let tinted = |renderer: &mut Renderer, mask: u32, farthest: f32, near_layers: u32| {
            let effects = vec![
                EffectInstance::new(tint)
                    .set("mask", mask)
                    .set("amount", 1.0)
                    .set("color", [1.0, 0.0, 0.0])
                    .set("farthest", farthest),
            ];
            let look = placed(effects, Vec::new());
            let mut viewport = renderer
                .three()
                .create_viewport(
                    ViewportConfig::new(SIZE, SIZE)
                        .samples(1)
                        .antialiasing(antialiasing),
                )
                .expect("a viewport");
            draw(renderer, &mut viewport, &frame(&scene, look, near_layers)).expect("frame draws")
        };

        let by_layer = tinted(&mut renderer, 2, 0.0, 2);
        assert!(
            red(&by_layer, near),
            "{antialiasing:?}: {:?}",
            rgb(&by_layer, near.0, near.1)
        );
        assert!(
            !red(&by_layer, far),
            "{antialiasing:?}: {:?}",
            rgb(&by_layer, far.0, far.1)
        );

        let everywhere = tinted(&mut renderer, 1 | 2, 0.0, 2);
        assert!(
            red(&everywhere, near) && red(&everywhere, far),
            "{antialiasing:?}"
        );

        let by_depth = tinted(&mut renderer, 1 | 2, 0.02, 2);
        assert!(
            red(&by_depth, near),
            "{antialiasing:?}: {:?}",
            rgb(&by_depth, near.0, near.1)
        );
        assert!(
            !red(&by_depth, far),
            "{antialiasing:?}: {:?}",
            rgb(&by_depth, far.0, far.1)
        );
    }
}

#[test]
fn runtime_slang_effect_sees_normals_and_large_parameter_blocks() {
    let mut renderer = renderer();
    let compiler = compiler();
    let scene = wall(&mut renderer);
    let effect = create(&mut renderer, &probe(&compiler));
    let schema = renderer
        .three()
        .effect_schema(effect)
        .expect("the effect exists");
    assert!(
        schema.size() > 128,
        "parameter block is {} bytes",
        schema.size()
    );

    let shown = |renderer: &mut Renderer, mode: u32| {
        let effects = vec![EffectInstance::new(effect).set("mode", mode)];
        render(renderer, 1, &frame(&scene, look(effects), 1)).expect("frame draws")
    };
    let normals = shown(&mut renderer, 1);
    let [r, g, b] = rgb(&normals, SIZE / 2 + 30, SIZE / 2);
    assert!(
        (r - 128).abs() <= 3 && (g - 128).abs() <= 3 && b >= 250,
        "a wall facing +Z reads {r} {g} {b}"
    );

    let marked = shown(&mut renderer, 4);
    let [r, g, b] = rgb(&marked, SIZE / 2, SIZE / 2);
    assert!(
        (r - 64).abs() <= 2 && (g - 128).abs() <= 2 && (b - 191).abs() <= 2,
        "the parameter behind 144 bytes reads {r} {g} {b}"
    );
}

#[test]
fn velocity_and_history_follow_the_previous_frame() {
    let mut renderer = renderer();
    let compiler = compiler();
    let scene = wall(&mut renderer);
    let effect = create(&mut renderer, &probe(&compiler));
    let probe = |mode: u32| {
        vec![
            EffectInstance::new(effect)
                .set("mode", mode)
                .set("gain", 40.0),
        ]
    };
    let moved = |look: Look, shift: f32| {
        let mut frame = frame(&scene, look, 1);
        *frame.camera_mut() = Camera::look_at(
            Vec3::new(shift, 0.0, 3.0),
            Vec3::new(shift, 0.0, 0.0),
            Vec3::Y,
            1.0,
            1.0,
            0.05,
        );
        frame
    };
    let at = (SIZE / 2 + 30, SIZE / 2);

    let mut panning = viewport(&mut renderer, 1);
    let still = draw(&mut renderer, &mut panning, &moved(look(probe(2)), 0.0)).expect("draws");
    assert_eq!(rgb(&still, at.0, at.1)[0], 0, "a first frame has no motion");
    let panned = draw(&mut renderer, &mut panning, &moved(look(probe(2)), 0.2)).expect("draws");
    assert!(
        rgb(&panned, at.0, at.1)[0] > 60,
        "a panned camera moves the wall: {:?}",
        rgb(&panned, at.0, at.1)
    );

    let mut blending = viewport(&mut renderer, 1);
    let first = draw(
        &mut renderer,
        &mut blending,
        &frame(&scene, look(probe(3)), 1),
    )
    .expect("draws");
    let lit = luma(&first, at.0, at.1);
    assert!(lit > 40, "the wall shows on the first frame: {lit}");
    let mut empty = Frame::new(Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        1.0,
        1.0,
        0.05,
    ));
    empty
        .background(Background::Color(Vec3::ZERO))
        .outline(rikka_renderer_3d::Outline {
            enabled: false,
            ..Default::default()
        })
        .look(look(probe(3)));
    let second = draw(&mut renderer, &mut blending, &empty).expect("draws");
    let blended = luma(&second, at.0, at.1);
    assert!(
        (blended - lit / 2).abs() <= 4,
        "history blends half of the last frame: {blended} vs {lit}"
    );
}

#[test]
fn disabled_effects_leave_the_frame_untouched() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let effects = [
        BuiltinEffect::Vignette.effect(),
        BuiltinEffect::FilmGrain.effect(),
        BuiltinEffect::LayerTint.effect(),
    ];
    let ids: Vec<EffectId> = effects
        .into_iter()
        .map(|effect| create(&mut renderer, &effect))
        .collect();
    let plain = render(&mut renderer, 4, &frame(&scene, look(Vec::new()), 1)).expect("frame draws");
    let effects = ids
        .into_iter()
        .map(|effect| EffectInstance::new(effect).enabled(false))
        .collect();
    let disabled = render(&mut renderer, 4, &frame(&scene, look(effects), 1)).expect("frame draws");
    assert!(plain == disabled, "disabled effects changed pixels");
}

const RESAMPLE: &str = r#"
import rikka;

struct Push {
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void resample(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    bool sized = all(rikka_effect_image_size(push.source) == push.size);
    float4 sampled = rikka_effect_sample(push.source, rikka_effect_uv(pixel, push.size));
    rikka_effect_store(push.target, pixel, sized ? sampled : float4(1.0, 0.0, 1.0, 1.0));
}
"#;

#[test]
fn effects_sample_their_input_while_the_scale_steps() {
    const FRAMES: u32 = 40;
    let mut renderer = renderer();
    let compiler = compiler();
    let scene = wall(&mut renderer);
    let code = compiler
        .effect_code("resample", RESAMPLE, "resample")
        .expect("the resampling effect compiles");
    let effect = create(
        &mut renderer,
        &Effect::new("resample").input(Composite::IMAGE).step(
            EffectStep::new(code)
                .read("source", image())
                .write("target", EffectOutput::Image)
                .uniform("size", rikka_renderer_3d::EffectUniform::Size),
        ),
    );
    let stepping = Antialiasing::Upscale(Upscale {
        scale: RenderScale::Dynamic(
            DynamicScale::new(std::time::Duration::from_nanos(1)).range(0.25, 1.0),
        ),
        sharpness: 0.0,
    });
    let shown = |renderer: &mut Renderer, effects: Vec<EffectInstance>| {
        let mut viewport = renderer
            .three()
            .create_viewport(
                ViewportConfig::new(SIZE, SIZE)
                    .samples(1)
                    .antialiasing(stepping),
            )
            .expect("a viewport");
        let frame = frame(&scene, look(effects), 1);
        let mut scales = Vec::new();
        let mut pixels = Vec::new();
        for _ in 0..FRAMES {
            pixels = draw(renderer, &mut viewport, &frame).expect("frame draws");
            scales.push(viewport.render_scale());
        }
        (scales, pixels)
    };
    let (plain_scales, plain) = shown(&mut renderer, Vec::new());
    let (scales, resampled) = shown(&mut renderer, vec![EffectInstance::new(effect)]);
    assert_eq!(scales, plain_scales, "both viewports step alike");
    assert_eq!(scales.last(), Some(&0.25), "the scale steps down");
    let worst = plain
        .iter()
        .zip(&resampled)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(
        worst <= 2,
        "resampling the drawn area changes a channel by {worst}"
    );
}

const PIPELINE_BUILD: std::time::Duration = std::time::Duration::from_secs(30);

const SOLID: &str = r#"
import rikka;

struct Push {
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void fill(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    rikka_effect_store(push.target, int2(id.xy), float4(COLOR, 1.0));
}
"#;

fn solid(compiler: &ShaderCompiler, color: &str) -> Effect {
    let source = SOLID.replace("COLOR", color);
    let code = compiler
        .effect_code("solid", &source, "fill")
        .expect("the solid effect compiles");
    Effect::new("solid").input(Composite::IMAGE).step(
        EffectStep::new(code)
            .write("target", EffectOutput::Image)
            .uniform("size", rikka_renderer_3d::EffectUniform::Size),
    )
}

#[test]
fn reloaded_effects_draw_their_old_program_until_the_new_one_is_ready() {
    let mut renderer = renderer();
    let compiler = compiler();
    let scene = wall(&mut renderer);
    let id = create(&mut renderer, &solid(&compiler, "1.0, 0.0, 0.0"));
    let readiness = renderer
        .three()
        .effect_readiness(id)
        .expect("the effect exists");
    let (sender, built) = std::sync::mpsc::channel();
    let ready = sender.clone();
    readiness.on_ready(move || ready.send(()).expect("the test waits"));
    built
        .recv_timeout(PIPELINE_BUILD)
        .expect("the first program is built");
    let first = readiness.generation();
    let shown = |renderer: &mut Renderer| {
        let pixels = render(
            renderer,
            1,
            &frame(&scene, look(vec![EffectInstance::new(id)]), 1),
        )
        .expect("frame draws");
        rgb(&pixels, SIZE / 2, SIZE / 2)
    };
    assert_eq!(shown(&mut renderer), [255, 0, 0]);

    renderer
        .three()
        .reload_effect(id, &solid(&compiler, "0.0, 0.0, 1.0"))
        .expect("the new program is valid");
    assert!(
        readiness.generation() > first,
        "a reload starts a new generation"
    );
    readiness.on_ready(move || sender.send(()).expect("the test waits"));
    let during = shown(&mut renderer);
    match readiness.is_ready() {
        false => assert_eq!(during, [255, 0, 0], "the old program draws while building"),
        true => assert!(during == [255, 0, 0] || during == [0, 0, 255], "{during:?}"),
    }
    built
        .recv_timeout(PIPELINE_BUILD)
        .expect("the reloaded program is built");
    assert!(readiness.is_ready());
    assert_eq!(
        shown(&mut renderer),
        [0, 0, 255],
        "the new program draws once ready"
    );
}

const INVERT: &str = r#"
import rikka;

struct Push {
    uint source;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("fragment")]
float4 invert(float4 frag_coord: SV_Position) : SV_Target {
    float4 scene = rikka_effect_load(push.source, int2(frag_coord.xy));
    return float4(1.0 - scene.rgb, 1.0);
}
"#;

#[test]
fn fragment_effects_draw_their_output() {
    let mut renderer = renderer();
    let compiler = compiler();
    let scene = wall(&mut renderer);
    let code = compiler
        .effect_code("invert", INVERT, "invert")
        .expect("the fragment effect compiles");
    assert_eq!(code.stage(), EffectStage::Fragment);
    let effect = Effect::new("invert").input(Composite::IMAGE).step(
        EffectStep::new(code)
            .read("source", image())
            .output(EffectOutput::Image),
    );
    let invert = create(&mut renderer, &effect);
    let plain = render(&mut renderer, 1, &frame(&scene, look(Vec::new()), 1)).expect("frame draws");
    let inverted = render(
        &mut renderer,
        1,
        &frame(&scene, look(vec![EffectInstance::new(invert)]), 1),
    )
    .expect("frame draws");
    for (x, y) in [(4, 4), (SIZE / 2 + 20, SIZE / 2), (SIZE / 2 - 30, SIZE / 2)] {
        let expected = rgb(&plain, x, y).map(|channel| 255 - channel);
        let drawn = rgb(&inverted, x, y);
        assert!(
            drawn
                .iter()
                .zip(expected)
                .all(|(drawn, expected)| (drawn - expected).abs() <= 2),
            "({x}, {y}): {drawn:?} vs {expected:?}"
        );
    }
}

#[test]
fn effect_schema_comes_from_slang_reflection() {
    let mut renderer = renderer();
    let vignette = create(&mut renderer, &BuiltinEffect::Vignette.effect());
    let schema = renderer
        .three()
        .effect_schema(vignette)
        .expect("the effect exists");
    let kinds: Vec<(&str, ParameterKind)> = schema
        .parameters()
        .iter()
        .map(|parameter| (parameter.name.as_str(), parameter.kind.clone()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("center", ParameterKind::Vec2),
            ("strength", ParameterKind::Float),
            ("radius", ParameterKind::Float),
            ("softness", ParameterKind::Float),
            ("roundness", ParameterKind::Float),
            ("squareness", ParameterKind::Float),
            (
                "color",
                ParameterKind::Color {
                    alpha: false,
                    srgb: false
                }
            ),
        ]
    );
    let strength = schema.parameter("strength").expect("strength exists");
    assert_eq!(strength.default, Literal::Float(0.5));
    assert_eq!(
        strength.range,
        Some(Bounds {
            low: 0.0,
            high: 1.0
        })
    );
    let radius = schema.parameter("radius").expect("radius exists");
    assert_eq!(radius.group.as_deref(), Some("shape"));
}

#[test]
fn invalid_effects_are_refused() {
    let mut renderer = renderer();
    let compiler = compiler();
    let code = compiler
        .effect_code("probe", PROBE, "probe")
        .expect("the probe compiles");
    let step = || EffectStep::new(code.clone());
    let complete = |output: EffectOutput| {
        step()
            .read("source", image())
            .read("normals", EffectInput::Normals)
            .read("velocity", EffectInput::Velocity)
            .read("history", EffectInput::History)
            .write("target", output)
            .uniform("size", rikka_renderer_3d::EffectUniform::Size)
    };
    let refused: [(&str, Effect); 8] = [
        (
            "unknown member",
            Effect::new("member").input(Composite::IMAGE).step(
                step()
                    .read("missing", image())
                    .write("target", EffectOutput::Image),
            ),
        ),
        (
            "unbound push member",
            Effect::new("unbound").input(Composite::IMAGE).step(
                step()
                    .read("source", image())
                    .write("target", EffectOutput::Image),
            ),
        ),
        (
            "image bound to a vector",
            Effect::new("type").input(Composite::IMAGE).step(
                step()
                    .read("size", image())
                    .write("target", EffectOutput::Image),
            ),
        ),
        (
            "target read before written",
            Effect::new("order")
                .input(Composite::IMAGE)
                .target("blur", Default::default())
                .step(
                    step()
                        .read("source", EffectInput::Target("blur".into()))
                        .write("target", EffectOutput::Image),
                ),
        ),
        (
            "undeclared input",
            Effect::new("socket").step(complete(EffectOutput::Image)),
        ),
        (
            "input declared twice",
            Effect::new("twice")
                .input(Composite::IMAGE)
                .input(Composite::IMAGE)
                .step(complete(EffectOutput::Image)),
        ),
        (
            "image written twice",
            Effect::new("again")
                .input(Composite::IMAGE)
                .step(complete(EffectOutput::Image))
                .step(complete(EffectOutput::Image)),
        ),
        (
            "image never written",
            Effect::new("silent")
                .input(Composite::IMAGE)
                .target("kept", Default::default())
                .step(complete(EffectOutput::Target("kept".into()))),
        ),
    ];
    for (case, effect) in refused {
        let created = renderer.three().create_effect(&effect);
        assert!(
            matches!(created, Err(Error::Invalid(_) | Error::InvalidShader(_))),
            "{case}: {created:?}"
        );
    }
    let broken = compiler.effect_code(
        "broken",
        "import rikka; [shader(\"compute\")] [numthreads(8, 8, 1)] void main() { undeclared(); }",
        "main",
    );
    assert!(matches!(broken, Err(Error::Slang(_))), "{broken:?}");

    let scene = wall(&mut renderer);
    let tint = create(&mut renderer, &BuiltinEffect::LayerTint.effect());
    let rejected = [
        EffectInstance::new(tint).set("missing", 1.0),
        EffectInstance::new(tint).set("amount", [1.0, 0.0]),
    ];
    for instance in rejected {
        let drawn = render(
            &mut renderer,
            1,
            &frame(&scene, look(vec![instance.clone()]), 1),
        );
        assert!(
            matches!(drawn, Err(Error::Parameter(_))),
            "{instance:?}: {drawn:?}"
        );
    }
    assert!(
        renderer
            .three()
            .destroy_effect(tint)
            .expect("meshes are enabled")
    );
    let gone = render(
        &mut renderer,
        1,
        &frame(&scene, look(vec![EffectInstance::new(tint)]), 1),
    );
    assert!(matches!(gone, Err(Error::Unknown(_))), "{gone:?}");
}
