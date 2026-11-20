use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::{Mat4, Vec3};
use rikka_renderer_3d::{
    Background, Composite, CompositeInput, CompositeNode, Draw, Effect, EffectId, EffectInput,
    EffectInstance, EffectOutput, EffectStep, EffectUniform, Error, Exposure, Figure, Frame, Grade,
    Lens, Light, Look, MaterialId, MeshId, Metering, Operation, RenderLayer, ShaderCompiler,
    ShadowQuality, Three, Tonemap,
};

use crate::support::{SIZE, ambient_frame, flat, quad, read, renderer, viewport};

const IMAGE: (&str, &str) = ("source", Composite::IMAGE);

const SPLIT: &str = r#"
import rikka;

struct Push {
    uint left;
    uint right;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void split(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    uint chosen = id.x < push.size.x / 2u ? push.left : push.right;
    rikka_effect_store(push.target, pixel, rikka_effect_load(chosen, pixel));
}
"#;

const INVERT: &str = r#"
import rikka;

struct Push {
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void invert(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float4 color = rikka_effect_load(push.source, pixel);
    rikka_effect_store(push.target, pixel, float4(1.0 - color.rgb, 1.0));
}
"#;

const ECHO: &str = r#"
import rikka;

struct Push {
    uint source;
    uint history;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void echo(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float4 now = rikka_effect_load(push.source, pixel);
    float4 before = rikka_effect_load(push.history, pixel);
    rikka_effect_store(push.target, pixel, lerp(now, before, 0.5));
}
"#;

const TRAIL: &str = r#"
import rikka;

struct Parameters {
    [Default(0.5)] [Range(0.0, 1.0)] float decay;
};

struct Push {
    Parameters* parameters;
    uint image;
    uint previous;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void trail(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float4 now = rikka_effect_load(push.image, pixel);
    float4 before = rikka_effect_load(push.previous, pixel);
    rikka_effect_store(push.target, pixel, max(now, before * push.parameters.decay));
}
"#;

fn plain_look() -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    rikka_test_renderer_3d::sky_of(&mut look).hour = 0.0;
    rikka_test_renderer_3d::sky_of(&mut look).moon = 0.0;
    rikka_test_renderer_3d::exposure_mut(&mut look).adaptation = 0.0;
    look
}

fn lit_frame(look: Look, light: f32, time: f32) -> Frame {
    let mut frame = ambient_frame(0.0);
    lit(&mut frame, look, light, time);
    frame
}

fn lit(frame: &mut Frame, look: Look, light: f32, time: f32) {
    frame
        .background(Background::Color(Vec3::ZERO))
        .ambient(Vec3::ZERO)
        .time(time)
        .look(look);
    frame.light(
        Light::directional(-Vec3::Z)
            .intensity(light)
            .shadow(ShadowQuality::None),
    );
}

fn luma(pixels: &[u8], x: u32, y: u32) -> f32 {
    let at = ((y * SIZE + x) * 4) as usize;
    0.2126 * f32::from(pixels[at])
        + 0.7152 * f32::from(pixels[at + 1])
        + 0.0722 * f32::from(pixels[at + 2])
}

struct Scene {
    wall: MeshId,
    grey: MaterialId,
}

fn wall(renderer: &mut Renderer) -> Scene {
    Scene {
        wall: quad(renderer, 0.0, 4.0),
        grey: flat(renderer, [0.5, 0.5, 0.5, 1.0]),
    }
}

fn render_wall(
    renderer: &mut Renderer,
    scene: &Scene,
    look: Look,
    light: f32,
    time: f32,
) -> Vec<u8> {
    let mut viewport = viewport(renderer, 1);
    let mut frame = lit_frame(look, light, time);
    frame.draw(Draw::new(scene.wall, scene.grey));
    read(renderer, &mut viewport, &frame)
}

fn attempt(renderer: &mut Renderer, scene: &Scene, look: Look) -> Result<(), Error> {
    let mut viewport = viewport(renderer, 1);
    let mut frame = lit_frame(look, 1.0, 0.0);
    frame.draw(Draw::new(scene.wall, scene.grey));
    renderer.three().render(&mut viewport, &frame)
}

fn effect(
    renderer: &mut Renderer,
    source: &str,
    entry: &str,
    sockets: &[(&str, &str)],
    history: bool,
) -> EffectId {
    let compiler = ShaderCompiler::new().expect("libslang loads");
    let code = compiler
        .effect_code(entry, source, entry)
        .expect("the test effect compiles");
    let mut step = EffectStep::new(code)
        .write("target", EffectOutput::Image)
        .uniform("size", EffectUniform::Size);
    let mut effect = Effect::new(entry);
    for &(member, socket) in sockets {
        step = step.read(member, EffectInput::Input(socket.to_owned()));
        effect = effect.input(socket);
    }
    if history {
        step = step.read("history", EffectInput::History);
    }
    renderer
        .three()
        .create_effect(&effect.step(step))
        .expect("the test effect is valid")
}

fn graded(composite: &mut Composite) -> usize {
    composite.push(Exposure {
        adaptation: 0.0,
        ..Exposure::default()
    });
    composite.push(Grade::default());
    composite.push(Tonemap::default())
}

fn with(composite: Composite) -> Look {
    Look {
        composite,
        ..plain_look()
    }
}

#[test]
fn look_post_chain_writes_output() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let pixels = render_wall(&mut renderer, &scene, Look::default(), 3.0, 0.0);
    let center = luma(&pixels, SIZE / 2, SIZE / 2);
    assert!(center > 40.0 && center < 250.0, "{center}");
    assert_eq!(pixels[((SIZE / 2 * SIZE + SIZE / 2) * 4 + 3) as usize], 255);
}

#[test]
fn auto_exposure_narrows_brightness_gap() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let fixed = plain_look();
    let mut automatic = plain_look();
    rikka_test_renderer_3d::exposure_mut(&mut automatic).adaptation = 1.0;
    let center = SIZE / 2;
    let mut shown = |look: &Look, light: f32| {
        luma(
            &render_wall(&mut renderer, &scene, look.clone(), light, 0.0),
            center,
            center,
        )
    };
    let fixed_dark = shown(&fixed, 0.05);
    let fixed_bright = shown(&fixed, 5.0);
    let auto_dark = shown(&automatic, 0.05);
    let auto_bright = shown(&automatic, 5.0);
    assert!(fixed_bright - fixed_dark > auto_bright - auto_dark + 30.0);
    assert!(auto_dark > fixed_dark + 20.0, "{auto_dark} {fixed_dark}");
    assert!(auto_bright > auto_dark, "night stays darker than day");
}

#[test]
fn manual_exposure_scales_by_its_compensation() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let manual = |compensation: f32| {
        let mut look = plain_look();
        *rikka_test_renderer_3d::exposure_mut(&mut look) = Exposure {
            compensation,
            metering: Metering::Manual,
            ..Exposure::default()
        };
        look
    };
    let center = SIZE / 2;
    let even = luma(
        &render_wall(&mut renderer, &scene, manual(0.0), 1.0, 0.0),
        center,
        center,
    );
    let fixed = luma(
        &render_wall(&mut renderer, &scene, plain_look(), 1.0, 0.0),
        center,
        center,
    );
    let brighter = luma(
        &render_wall(&mut renderer, &scene, manual(1.0), 1.0, 0.0),
        center,
        center,
    );
    assert!((even - fixed).abs() <= 2.0, "{even} vs {fixed}");
    assert!(brighter > even + 20.0, "{brighter} vs {even}");
}

#[test]
fn figure_metering_keeps_figures_from_darkening_exposure() {
    let mut renderer = renderer();
    let wall = quad(&mut renderer, 0.0, 4.0);
    let dim = flat(&mut renderer, [0.1, 0.1, 0.1, 1.0]);
    let card = quad(&mut renderer, 0.1, 1.5);
    let white = flat(&mut renderer, [1.0, 1.0, 1.0, 1.0]);
    let mut corner = |figure_metering: f32| {
        let mut look = plain_look();
        let exposure = rikka_test_renderer_3d::exposure_mut(&mut look);
        exposure.adaptation = 1.0;
        exposure.figure_metering = figure_metering;
        let mut viewport = viewport(&mut renderer, 1);
        let mut frame = lit_frame(look, 3.0, 0.0);
        let figure = frame.figure(Figure::default());
        frame.draw(Draw::new(wall, dim));
        frame.draw(Draw::new(card, white).figure(figure));
        luma(&read(&mut renderer, &mut viewport, &frame), 6, 6)
    };
    let counted = corner(1.0);
    let excluded = corner(0.0);
    assert!(excluded > counted + 10.0, "{excluded} {counted}");
}

#[test]
fn a_multi_input_effect_mixes_two_branches() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let invert = effect(&mut renderer, INVERT, "invert", &[IMAGE], false);
    let split = effect(
        &mut renderer,
        SPLIT,
        "split",
        &[("left", "left"), ("right", "right")],
        false,
    );
    let mut composite = Composite::new();
    let toned = graded(&mut composite);
    let inverted = composite.push(EffectInstance::new(invert));
    let mixed = composite.node(
        EffectInstance::new(split),
        [
            ("left", CompositeInput::Node(toned)),
            ("right", CompositeInput::Node(inverted)),
        ],
    );
    composite.output(CompositeInput::Node(mixed));
    let plain = render_wall(&mut renderer, &scene, plain_look(), 2.0, 0.0);
    let both = render_wall(&mut renderer, &scene, with(composite), 2.0, 0.0);
    let row = SIZE / 2;
    for x in [8, SIZE / 4, SIZE / 2 - 4] {
        let (kept, shown) = (luma(&plain, x, row), luma(&both, x, row));
        assert!((kept - shown).abs() <= 2.0, "left {x}: {shown} vs {kept}");
    }
    for x in [SIZE / 2 + 4, 3 * SIZE / 4, SIZE - 8] {
        let (kept, shown) = (luma(&plain, x, row), luma(&both, x, row));
        assert!(
            (255.0 - kept - shown).abs() <= 3.0,
            "right {x}: {shown} vs the inverse of {kept}"
        );
    }
}

#[test]
fn invalid_composites_are_refused() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let split = effect(
        &mut renderer,
        SPLIT,
        "split",
        &[("left", "left"), ("right", "right")],
        false,
    );
    let image = CompositeInput::Layer(RenderLayer::Image);
    let node = |key: u64, operation: Operation, inputs: &[(&str, CompositeInput)]| CompositeNode {
        key,
        operation,
        inputs: inputs
            .iter()
            .map(|&(name, input)| (name.to_owned(), input))
            .collect(),
    };
    let tone = || Operation::Tonemap(Tonemap::default());
    let composite = |nodes: Vec<CompositeNode>| Composite {
        nodes,
        output: CompositeInput::Node(0),
    };
    let cases = [
        (
            "a link forward",
            composite(vec![
                node(0, tone(), &[(Composite::IMAGE, CompositeInput::Node(1))]),
                node(1, tone(), &[(Composite::IMAGE, image)]),
            ]),
        ),
        (
            "a link to itself",
            composite(vec![node(
                0,
                tone(),
                &[(Composite::IMAGE, CompositeInput::Node(0))],
            )]),
        ),
        (
            "an unknown socket",
            composite(vec![node(
                0,
                tone(),
                &[(Composite::IMAGE, image), ("colour", image)],
            )]),
        ),
        ("a missing image", composite(vec![node(0, tone(), &[])])),
        (
            "a socket wired twice",
            composite(vec![node(
                0,
                tone(),
                &[(Composite::IMAGE, image), (Composite::IMAGE, image)],
            )]),
        ),
        (
            "a key used twice",
            composite(vec![
                node(3, tone(), &[(Composite::IMAGE, image)]),
                node(3, tone(), &[(Composite::IMAGE, CompositeInput::Node(0))]),
            ]),
        ),
        (
            "an output beyond the nodes",
            Composite {
                output: CompositeInput::Node(4),
                ..composite(vec![node(0, tone(), &[(Composite::IMAGE, image)])])
            },
        ),
        (
            "an unwired effect input",
            composite(vec![node(
                0,
                Operation::Effect(EffectInstance::new(split)),
                &[("left", image)],
            )]),
        ),
    ];
    for (case, composite) in cases {
        let drawn = attempt(&mut renderer, &scene, with(composite));
        assert!(matches!(drawn, Err(Error::Invalid(_))), "{case}: {drawn:?}");
    }
    assert!(
        renderer
            .three()
            .destroy_effect(split)
            .expect("meshes are enabled")
    );
    let mut gone = Composite::new();
    graded(&mut gone);
    gone.push(EffectInstance::new(split));
    let drawn = attempt(&mut renderer, &scene, with(gone));
    assert!(matches!(drawn, Err(Error::Unknown(_))), "{drawn:?}");
}

#[test]
fn unreachable_nodes_are_left_out() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let invert = effect(&mut renderer, INVERT, "invert", &[IMAGE], false);
    let plain = render_wall(&mut renderer, &scene, plain_look(), 2.0, 0.0);
    let mut composite = Composite::new();
    let toned = graded(&mut composite);
    composite.push(EffectInstance::new(invert).set("missing", 1.0));
    composite.push(Lens::default());
    composite.output(CompositeInput::Node(toned));
    let pruned = render_wall(&mut renderer, &scene, with(composite.clone()), 2.0, 0.0);
    assert!(plain == pruned, "unreachable nodes changed the image");
    composite.output(CompositeInput::Node(composite.nodes.len() - 1));
    let reached = attempt(&mut renderer, &scene, with(composite));
    assert!(matches!(reached, Err(Error::Parameter(_))), "{reached:?}");
}

#[test]
fn effect_history_follows_the_node_key_across_reordering() {
    let mut renderer = renderer();
    let scene = wall(&mut renderer);
    let echo = effect(&mut renderer, ECHO, "echo", &[IMAGE], true);
    let chain = |echo_key: u64, padded: bool| {
        let mut composite = Composite::new();
        if padded {
            composite.push(Lens::default());
            composite.nodes[0].key = 100;
            composite.nodes[0].inputs[0].1 = CompositeInput::Layer(RenderLayer::Image);
        }
        let start = composite.nodes.len();
        composite.node(
            Exposure {
                adaptation: 0.0,
                ..Exposure::default()
            },
            [(Composite::IMAGE, CompositeInput::Layer(RenderLayer::Image))],
        );
        composite.push(Grade::default());
        composite.push(Tonemap::default());
        let echoed = composite.push(EffectInstance::new(echo));
        for (offset, node) in composite.nodes[start..].iter_mut().enumerate() {
            node.key = offset as u64;
        }
        composite.nodes[echoed].key = echo_key;
        with(composite)
    };
    let at = (SIZE / 2, SIZE / 2);
    let blended = |renderer: &mut Renderer, second: Look| {
        let mut viewport = viewport(renderer, 1);
        let mut first = lit_frame(chain(7, false), 2.0, 0.0);
        first.draw(Draw::new(scene.wall, scene.grey));
        let lit = luma(&read(renderer, &mut viewport, &first), at.0, at.1);
        let empty = lit_frame(second, 2.0, 0.0);
        let shown = luma(&read(renderer, &mut viewport, &empty), at.0, at.1);
        (lit, shown)
    };
    let (lit, kept) = blended(&mut renderer, chain(7, true));
    assert!(lit > 40.0, "the wall shows: {lit}");
    assert!(
        (kept - lit * 0.5).abs() <= 6.0,
        "a moved node keeps its history: {kept} vs {lit}"
    );
    let (_, fresh) = blended(&mut renderer, chain(8, true));
    assert!(fresh <= 3.0, "a new key starts a new history: {fresh}");
}

#[test]
fn previous_output_leaves_a_decaying_trail_until_a_cut() {
    let mut renderer = renderer();
    let card = quad(&mut renderer, 0.0, 0.3);
    let grey = flat(&mut renderer, [0.5, 0.5, 0.5, 1.0]);
    let trail = effect(
        &mut renderer,
        TRAIL,
        "trail",
        &[("image", Composite::IMAGE), ("previous", "previous")],
        false,
    );
    let mut composite = Composite::new();
    let toned = graded(&mut composite);
    composite.node(
        EffectInstance::new(trail),
        [
            (Composite::IMAGE, CompositeInput::Node(toned)),
            ("previous", CompositeInput::Layer(RenderLayer::Previous)),
        ],
    );
    composite.output(CompositeInput::Node(toned + 1));
    let look = with(composite);
    let mut viewport = viewport(&mut renderer, 1);
    let mut shown = |renderer: &mut Renderer, x: f32, cut: bool| {
        let mut frame = lit_frame(look.clone(), 2.0, 0.0);
        let moved = Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
        frame.draw(Draw::new(card, grey).transform(moved));
        if cut {
            frame.cut();
        }
        read(renderer, &mut viewport, &frame)
    };
    let left = 44;
    let row = SIZE / 2;
    let start = shown(&mut renderer, -0.5, false);
    let lit = luma(&start, left, row);
    assert!(lit > 60.0, "the card shows: {lit}");
    let once = luma(&shown(&mut renderer, 0.5, false), left, row);
    let twice = luma(&shown(&mut renderer, 0.5, false), left, row);
    assert!(
        (once - lit * 0.5).abs() <= 4.0,
        "a half trail: {once} of {lit}"
    );
    assert!(
        (twice - lit * 0.25).abs() <= 4.0,
        "a quarter trail: {twice} of {lit}"
    );
    let cut = luma(&shown(&mut renderer, 0.5, true), left, row);
    assert!(cut <= 2.0, "a cut clears the trail: {cut}");
}
