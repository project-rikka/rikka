use std::fmt::Write as _;

use rikka_color_lookup::ColorLookup;
use rikka_gpu::{Filtering, Pixels};
use rikka_renderer::{Node, Renderer, Scene, TextureId};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Mat4, Vec3, Vec4};
use rikka_renderer_3d::{
    Antialiasing, Background, BuiltinEffect, Composite, CompositeInput, Draw, Effect, EffectId,
    EffectInstance, EffectOutput, EffectStep, EffectUniform, Environment, Exposure, Frame, Glow,
    Grade, Light, Literal, Look, MaterialId, MaterialState, MeshId, NITS_PER_RADIANCE_UNIT,
    ParameterKind, RenderLayer, ShaderCompiler, ShadowQuality, Sky, TextureBinding, Tonemap,
    Upscale, Value, ViewportConfig,
};
use rikka_slang::ParameterSpec;
use rikka_types::{Color, Rect};

use crate::support::{ambient_frame, flat, quad, renderer};

const SIZE: u32 = 256;

const PATTERN: &str = r#"
import rikka;

struct Parameters {
    [Options("card|ramp|edge|noisy_edge|soft_edge|dots|grey_ramp")] uint pattern;
    [Default(1.0)] float gain;
};

struct Push {
    Parameters* parameters;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float noise(int2 pixel) {
    return rikka_effect_random(uint3(uint(pixel.x), uint(pixel.y), 77u)) - 0.5;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void paint(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    float2 extent = float2(push.size - 1u);
    float2 unit = float2(pixel) / extent;
    float3 rgb;
    switch (push.parameters.pattern) {
    case 0u: {
        bool checker = ((pixel.x / 16) + (pixel.y / 16)) % 2 == 0;
        rgb = float3(unit.x, unit.y, 1.0 - unit.x) * (checker ? 1.0 : 0.45) + noise(pixel) * 0.05;
        break;
    }
    case 1u:
        rgb = float3(unit.x, unit.y, 0.5 * (unit.x + unit.y));
        break;
    case 2u:
        rgb = float3(unit.x < 0.5 ? 0.25 : 0.75);
        break;
    case 3u:
        rgb = float3((unit.x < 0.5 ? 0.25 : 0.75) + noise(pixel) * 0.2);
        break;
    case 4u:
        rgb = float3(lerp(0.3, 0.7, smoothstep(-2.0, 2.0, float(pixel.x) - extent.x * 0.5)));
        break;
    case 5u: {
        bool corner = all(pixel >= int2(40)) && all(pixel < int2(56));
        bool middle = all(pixel >= int2(120)) && all(pixel < int2(136));
        rgb = float3(corner || middle ? 1.0 : 0.0);
        break;
    }
    default:
        rgb = float3(unit.x);
        break;
    }
    rikka_effect_store(push.target, pixel, float4(max(rgb, float3(0.0)) * push.parameters.gain, 1.0));
}
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Placement {
    Scene,
    Display,
}

type Placed = (Placement, EffectInstance);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pattern {
    Card,
    Ramp,
    NoisyEdge,
    SoftEdge,
    Dots,
    GreyRamp,
}

impl Pattern {
    fn option(self) -> &'static str {
        match self {
            Pattern::Card => "card",
            Pattern::Ramp => "ramp",
            Pattern::NoisyEdge => "noisy_edge",
            Pattern::SoftEdge => "soft_edge",
            Pattern::Dots => "dots",
            Pattern::GreyRamp => "grey_ramp",
        }
    }
}

struct Stage {
    renderer: rikka_test_renderer::TestRenderer,
    near: MeshId,
    far: MeshId,
    grey: MaterialId,
    glowing: MaterialId,
    pattern: EffectId,
}

type Image = Vec<u8>;

impl Stage {
    fn new() -> Stage {
        let mut renderer = renderer();
        let compiler = ShaderCompiler::new().expect("libslang loads");
        let code = compiler
            .effect_code("pattern", PATTERN, "paint")
            .expect("the pattern compiles");
        let effect = Effect::new("pattern").input(Composite::IMAGE).step(
            EffectStep::new(code)
                .write("target", EffectOutput::Image)
                .uniform("size", EffectUniform::Size),
        );
        let pattern = renderer
            .three()
            .create_effect(&effect)
            .expect("the pattern is valid");
        let near = quad(&mut renderer, 1.0, 0.4);
        let far = quad(&mut renderer, 0.0, 4.0);
        let grey = flat(&mut renderer, [0.6, 0.6, 0.6, 1.0]);
        let glowing = renderer
            .three()
            .create_material(
                rikka_test_renderer_3d::standard(
                    Vec4::new(0.6, 0.6, 0.6, 1.0),
                    MaterialState::opaque(),
                )
                .with("emission_color", Vec3::new(1.0, 0.3, 0.075))
                .and_then(|desc| desc.with("emission_luminance", 4.0 * NITS_PER_RADIANCE_UNIT))
                .expect("standard materials emit"),
            )
            .expect("a material");
        Stage {
            renderer,
            near,
            far,
            grey,
            glowing,
            pattern,
        }
    }

    fn create(&mut self, builtin: BuiltinEffect) -> (EffectId, Placement) {
        let id = self
            .renderer
            .three()
            .create_effect(&builtin.effect())
            .expect("built-in effects are valid");
        let placement = match builtin.is_display_referred() {
            true => Placement::Display,
            false => Placement::Scene,
        };
        (id, placement)
    }

    fn pattern(&self, pattern: Pattern, placement: Placement, gain: f32) -> Placed {
        let instance = EffectInstance::new(self.pattern)
            .set("pattern", Value::Enum(pattern.option().into()))
            .set("gain", gain);
        (placement, instance)
    }

    fn frame(&self, effects: Vec<Placed>, near_layers: u32, time: f32) -> Frame {
        self.scene(fixed_look(effects), near_layers, self.grey, time)
    }

    fn glowing_frame(&self, effects: Vec<Placed>, near_layers: u32, time: f32) -> Frame {
        self.scene(fixed_look(effects), near_layers, self.glowing, time)
    }

    fn scene(&self, look: Look, near_layers: u32, near: MaterialId, time: f32) -> Frame {
        let mut frame = ambient_frame(0.0);
        let background = match look.environment {
            Environment::Sky(_) => Background::Sky,
            Environment::None | Environment::Image(_) => Background::Color(Vec3::ZERO),
        };
        frame
            .background(background)
            .ambient(Vec3::splat(0.4))
            .time(time)
            .look(look);
        frame.light(
            Light::directional(-Vec3::Z)
                .intensity(2.0)
                .shadow(ShadowQuality::None),
        );
        frame.draw(Draw::new(self.far, self.grey).layers(1));
        frame.draw(
            Draw::new(self.near, near)
                .transform(Mat4::from_translation(Vec3::new(-0.5, 0.0, 0.0)))
                .layers(near_layers),
        );
        frame
    }

    fn shoot(&mut self, frame: &Frame) -> Image {
        self.shoot_with(frame, Antialiasing::Multisample)
    }

    fn shoot_with(&mut self, frame: &Frame, antialiasing: Antialiasing) -> Image {
        let renderer: &mut Renderer = &mut self.renderer;
        let mut viewport = renderer
            .three()
            .create_viewport(ViewportConfig::new(SIZE, SIZE).antialiasing(antialiasing))
            .expect("a viewport");
        renderer
            .three()
            .render(&mut viewport, frame)
            .expect("the frame draws");
        let mut target = renderer.create_target(SIZE, SIZE).expect("a target");
        let mut scene = Scene::new();
        scene.set_background(Color::rgb(0.0, 0.0, 0.0));
        scene.insert(Node::image(
            Rect::new(0.0, 0.0, SIZE as f32, SIZE as f32),
            viewport.texture(),
        ));
        renderer
            .draw(&mut scene, &mut target)
            .expect("the 2D frame draws");
        renderer.read_pixels(&mut target).expect("pixels come back")
    }

    fn lookup(&mut self, text: &str) -> TextureId {
        let parsed = ColorLookup::parse(text).expect("a valid table");
        self.renderer
            .uploads()
            .create_color_lookup(&parsed)
            .expect("the table uploads")
    }
}

fn composite(adaptation: f32, effects: Vec<Placed>) -> Composite {
    let mut composite = Composite::new();
    composite.push(Exposure {
        adaptation,
        ..Exposure::default()
    });
    let (scene, display): (Vec<_>, Vec<_>) = effects
        .into_iter()
        .partition(|(placement, _)| *placement == Placement::Scene);
    for (_, instance) in scene {
        composite.push(instance);
    }
    composite.push(Grade::default());
    composite.push(Tonemap::default());
    for (_, instance) in display {
        composite.push(instance);
    }
    composite
}

fn fixed_look(effects: Vec<Placed>) -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.environment = Environment::None;
    look.composite = composite(0.0, effects);
    look
}

fn metered_look(sky: bool, effects: Vec<Placed>) -> Look {
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.environment = match sky {
        true => Environment::Sky(Sky {
            clouds: 0.0,
            ..Sky::default()
        }),
        false => Environment::None,
    };
    look.composite = composite(1.0, effects);
    look
}

fn rgb(image: &[u8], x: u32, y: u32) -> [i32; 3] {
    let at = ((y * SIZE + x) * 4) as usize;
    [0, 1, 2].map(|channel| i32::from(image[at + channel]))
}

fn grey(image: &[u8], x: u32, y: u32) -> f32 {
    rgb(image, x, y)[1] as f32 / 255.0
}

fn mean_difference(a: &[u8], b: &[u8]) -> f64 {
    let sum: u64 = a
        .iter()
        .zip(b)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    sum as f64 / a.len() as f64
}

fn largest_difference(a: &[u8], b: &[u8]) -> u8 {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0)
}

fn cube(size: usize, map: impl Fn([f32; 3]) -> [f32; 3]) -> String {
    let mut text = format!("TITLE \"test\"\nLUT_3D_SIZE {size}\n");
    let last = (size - 1) as f32;
    for blue in 0..size {
        for green in 0..size {
            for red in 0..size {
                let [r, g, b] = map([red as f32 / last, green as f32 / last, blue as f32 / last]);
                writeln!(text, "{r:.7} {g:.7} {b:.7}").expect("write to String");
            }
        }
    }
    text
}

fn table(texture: TextureId) -> Value {
    Value::Texture(Some(TextureBinding::new(texture, Filtering::NearestClamp)))
}

#[test]
fn builtin_effects_compile_with_bounded_schemas() {
    let mut stage = Stage::new();
    for builtin in BuiltinEffect::ALL {
        assert_eq!(BuiltinEffect::from_name(builtin.name()), Some(builtin));
        let (id, _) = stage.create(builtin);
        let schema = stage
            .renderer
            .three()
            .effect_schema(id)
            .expect("the effect exists");
        assert!(
            !schema.parameters().is_empty(),
            "{builtin:?} has no parameters"
        );
        for spec in schema.parameters() {
            let numbers = match spec.default {
                Literal::Float(value) => vec![value],
                Literal::Vec2(values) => values.to_vec(),
                Literal::Vec3(values) => values.to_vec(),
                Literal::Vec4(values) => values.to_vec(),
                Literal::Int(value) => vec![value as f32],
                _ => Vec::new(),
            };
            let colour = matches!(spec.kind, ParameterKind::Color { .. });
            match spec.range {
                Some(range) => assert!(
                    numbers
                        .iter()
                        .all(|value| (range.low..=range.high).contains(value)),
                    "{builtin:?}.{}: default {:?} outside {range:?}",
                    spec.name,
                    spec.default
                ),
                None => assert!(
                    numbers.is_empty() || colour,
                    "{builtin:?}.{} has no range",
                    spec.name
                ),
            }
            if let (ParameterKind::Enum { options }, Literal::Enum(index)) =
                (&spec.kind, spec.default)
            {
                assert!(
                    (index as usize) < options.len(),
                    "{builtin:?}.{}",
                    spec.name
                );
            }
        }
    }
}

fn changed(spec: &ParameterSpec, dirt: TextureId) -> Value {
    let middle = |value: f32| match spec.range {
        Some(range) => 0.5 * (range.low + range.high) + 0.01 * (range.high - range.low),
        None => value + 0.25,
    };
    match (&spec.kind, spec.default) {
        (ParameterKind::Enum { options }, _) => {
            Value::Enum(options.last().expect("options").clone())
        }
        (ParameterKind::Texture { .. }, _) => table(dirt),
        (_, Literal::Float(value)) => Value::Float(middle(value)),
        (_, Literal::Vec2(values)) => Value::Vec2(values.map(middle).into()),
        (_, Literal::Vec3(values)) => Value::Vec3(values.map(middle).into()),
        (_, Literal::Vec4(values)) => Value::Vec4(values.map(middle).into()),
        (_, Literal::Int(value)) => Value::Int(match spec.range {
            Some(range) => ((range.low + range.high) * 0.5) as i32,
            None => value + 1,
        }),
        (_, Literal::UInt(value)) => Value::UInt(value + 1),
        (_, Literal::Bool(value)) => Value::Bool(!value),
        (kind, literal) => panic!("{}: unexpected {kind:?} {literal:?}", spec.name),
    }
}

#[test]
fn builtin_effects_draw_with_every_parameter_changed() {
    let mut stage = Stage::new();
    let dirt = stage
        .renderer
        .uploads()
        .create(Pixels::rgba8(1, 1, vec![255, 255, 255, 255]))
        .expect("a texture");
    for builtin in BuiltinEffect::ALL {
        let (id, placement) = stage.create(builtin);
        let schema = stage
            .renderer
            .three()
            .effect_schema(id)
            .expect("the effect exists");
        let instance = schema
            .parameters()
            .iter()
            .fold(EffectInstance::new(id), |instance, spec| {
                instance.set(spec.name.as_str(), changed(spec, dirt))
            });
        let shown = stage.frame(vec![(placement, instance)], 2, 0.5);
        stage.shoot(&shown);
    }
}

#[test]
fn defaults_change_the_image_unless_neutral() {
    let mut stage = Stage::new();
    let neutral = [
        BuiltinEffect::ColorLookup,
        BuiltinEffect::ChannelMixer,
        BuiltinEffect::Hsl,
        BuiltinEffect::Cdl,
        BuiltinEffect::WhiteBalance,
        BuiltinEffect::Trails,
        BuiltinEffect::FrameBlend,
        BuiltinEffect::Datamosh,
    ];
    for builtin in BuiltinEffect::ALL {
        let (id, insertion) = stage.create(builtin);
        let gain = match insertion {
            Placement::Display => 1.0,
            Placement::Scene => 4.0,
        };
        let pattern = stage.pattern(Pattern::Card, insertion, gain);
        let near_layers = match builtin {
            BuiltinEffect::LayerGlow => 2,
            _ => 1,
        };
        let plain = stage.glowing_frame(vec![pattern.clone()], near_layers, 1.25);
        let shaded = stage.glowing_frame(
            vec![pattern, (insertion, EffectInstance::new(id))],
            near_layers,
            1.25,
        );
        let before = stage.shoot(&plain);
        let after = stage.shoot(&shaded);
        let again = stage.shoot(&shaded);
        assert!(after == again, "{builtin:?} is not deterministic");
        let difference = mean_difference(&before, &after);
        match neutral.contains(&builtin) {
            true => assert!(
                largest_difference(&before, &after) <= 1,
                "{builtin:?} changes the image at its neutral defaults"
            ),
            false => assert!(
                difference > 0.02,
                "{builtin:?} changes the image by only {difference:.3}"
            ),
        }
    }
}

#[test]
fn color_lookup_identity_is_lossless_and_tables_map_exactly() {
    let mut stage = Stage::new();
    let (lookup, insertion) = stage.create(BuiltinEffect::ColorLookup);
    let ramp = stage.pattern(Pattern::Ramp, insertion, 1.0);
    let plain = stage.shoot(&stage.frame(vec![ramp.clone()], 1, 0.0));
    let identity_small = stage.lookup(&cube(17, |rgb| rgb));
    let identity_large = stage.lookup(&cube(65, |rgb| rgb));
    let swapped = stage.lookup(&cube(33, |[r, g, b]| [b, r, g]));
    let cases: [(&str, TextureId, &str, bool); 7] = [
        ("17³ identity", identity_small, "direct", false),
        ("65³ identity", identity_large, "direct", false),
        ("log shaper identity", identity_large, "log", false),
        ("ACEScct shaper identity", identity_large, "acescct", false),
        ("LogC3 shaper identity", identity_large, "logc3", false),
        ("sRGB shaper identity", identity_small, "srgb", false),
        ("33³ channel rotation", swapped, "direct", true),
    ];
    for (case, texture, shaper, rotated) in cases {
        let effects = vec![
            ramp.clone(),
            (
                insertion,
                EffectInstance::new(lookup)
                    .set("table", table(texture))
                    .set("shaper.mode", Value::Enum(shaper.into())),
            ),
        ];
        let looked = stage.shoot(&stage.frame(effects, 1, 0.0));
        for y in (0..SIZE).step_by(7) {
            for x in (0..SIZE).step_by(5) {
                let [r, g, b] = rgb(&plain, x, y);
                let expected = match rotated {
                    true => [b, r, g],
                    false => [r, g, b],
                };
                let found = rgb(&looked, x, y);
                let tolerance = match shaper {
                    "log" | "acescct" | "logc3" => 2,
                    _ => 1,
                };
                assert!(
                    expected
                        .iter()
                        .zip(found)
                        .all(|(e, f)| (e - f).abs() <= tolerance),
                    "{case} at {x},{y}: {found:?} instead of {expected:?}"
                );
            }
        }
    }
}

#[test]
fn color_lookup_reads_a_one_dimensional_strip() {
    let mut stage = Stage::new();
    let (lookup, insertion) = stage.create(BuiltinEffect::ColorLookup);
    let ramp = stage.pattern(Pattern::Ramp, insertion, 1.0);
    let plain = stage.shoot(&stage.frame(vec![ramp.clone()], 1, 0.0));
    let strip = stage
        .renderer
        .uploads()
        .create(Pixels::rgba8(2, 1, vec![255, 255, 255, 255, 0, 0, 0, 255]))
        .expect("a strip");
    let effects = vec![
        ramp,
        (
            insertion,
            EffectInstance::new(lookup).set("table", table(strip)),
        ),
    ];
    let inverted = stage.shoot(&stage.frame(effects, 1, 0.0));
    for y in (0..SIZE).step_by(9) {
        for x in (0..SIZE).step_by(7) {
            let expected = rgb(&plain, x, y).map(|value| 255 - value);
            let found = rgb(&inverted, x, y);
            assert!(
                expected.iter().zip(found).all(|(e, f)| (e - f).abs() <= 2),
                "{x},{y}: {found:?} instead of {expected:?}"
            );
        }
    }
}

fn centroid(image: &[u8], channel: usize, from: u32, to: u32) -> (f32, f32) {
    let mut sum = (0.0, 0.0, 0.0);
    for y in from..to {
        for x in from..to {
            let weight = f32::from(image[((y * SIZE + x) * 4) as usize + channel]);
            sum = (
                sum.0 + weight * x as f32,
                sum.1 + weight * y as f32,
                sum.2 + weight,
            );
        }
    }
    (sum.0 / sum.2.max(1.0), sum.1 / sum.2.max(1.0))
}

#[test]
fn chromatic_aberration_splits_channels_radially() {
    let mut stage = Stage::new();
    let (aberration, insertion) = stage.create(BuiltinEffect::ChromaticAberration);
    let dots = stage.pattern(Pattern::Dots, insertion, 4.0);
    let effects = vec![
        dots,
        (
            insertion,
            EffectInstance::new(aberration).set("amount", 60.0),
        ),
    ];
    let split = stage.shoot(&stage.frame(effects, 1, 0.0));
    let red = centroid(&split, 0, 24, 72);
    let blue = centroid(&split, 2, 24, 72);
    let centre = SIZE as f32 * 0.5;
    let distance = |(x, y): (f32, f32)| ((x - centre).powi(2) + (y - centre).powi(2)).sqrt();
    assert!(
        distance(blue) > distance(red) + 1.5,
        "the corner square splits radially: red {red:?}, blue {blue:?}"
    );
    let middle_red = centroid(&split, 0, 104, 152);
    let middle_blue = centroid(&split, 2, 104, 152);
    assert!(
        (middle_red.0 - middle_blue.0).abs() < 0.5 && (middle_red.1 - middle_blue.1).abs() < 0.5,
        "the centre stays aligned: red {middle_red:?}, blue {middle_blue:?}"
    );
}

fn deviation(image: &[u8], columns: std::ops::Range<u32>) -> f32 {
    let values: Vec<f32> = (32..224)
        .flat_map(|y| columns.clone().map(move |x| (x, y)))
        .map(|(x, y)| grey(image, x, y))
        .collect();
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f32>()
        / values.len() as f32)
        .sqrt()
}

fn column_profile(image: &[u8]) -> Vec<f32> {
    (0..SIZE)
        .map(|x| (32..224).map(|y| grey(image, x, y)).sum::<f32>() / 192.0)
        .collect()
}

#[test]
fn kuwahara_flattens_noise_and_keeps_edges() {
    let mut stage = Stage::new();
    let (kuwahara, insertion) = stage.create(BuiltinEffect::Kuwahara);
    let noisy = stage.pattern(Pattern::NoisyEdge, insertion, 1.0);
    let before = stage.shoot(&stage.frame(vec![noisy.clone()], 1, 0.0));
    let effects = vec![
        noisy,
        (insertion, EffectInstance::new(kuwahara).set("radius", 16.0)),
    ];
    let after = stage.shoot(&stage.frame(effects, 1, 0.0));
    let flat_before = deviation(&before, 16..100);
    let flat_after = deviation(&after, 16..100);
    assert!(
        flat_after < flat_before * 0.5,
        "noise {flat_before:.4} only fell to {flat_after:.4}"
    );
    let profile = column_profile(&after);
    let dark = profile
        .iter()
        .rposition(|value| *value < 0.375)
        .expect("a dark side");
    let bright = profile
        .iter()
        .position(|value| *value > 0.625)
        .expect("a bright side");
    assert!(
        bright.abs_diff(dark) <= 3,
        "the edge spreads from column {dark} to {bright}"
    );
}

#[test]
fn halftone_prints_binary_dots_following_tone() {
    let mut stage = Stage::new();
    let (halftone, insertion) = stage.create(BuiltinEffect::Halftone);
    let ramp = stage.pattern(Pattern::GreyRamp, insertion, 1.0);
    let effects = vec![
        ramp,
        (
            insertion,
            EffectInstance::new(halftone)
                .set("ink", [0.0, 0.0, 0.0])
                .set("paper", [1.0, 1.0, 1.0])
                .set("softness", 0.1),
        ),
    ];
    let printed = stage.shoot(&stage.frame(effects, 1, 0.0));
    let values: Vec<f32> = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .map(|(x, y)| grey(&printed, x, y))
        .collect();
    let binary = values
        .iter()
        .filter(|value| **value < 0.1 || **value > 0.9)
        .count() as f32
        / values.len() as f32;
    assert!(
        binary > 0.9,
        "only {:.1}% of pixels are ink or paper",
        binary * 100.0
    );
    let ink = |columns: std::ops::Range<u32>| {
        let count = columns.len() as f32 * SIZE as f32;
        (0..SIZE)
            .flat_map(|y| columns.clone().map(move |x| (x, y)))
            .map(|(x, y)| 1.0 - grey(&printed, x, y))
            .sum::<f32>()
            / count
    };
    let shadows = ink(0..64);
    let highlights = ink(192..256);
    assert!(
        shadows > highlights + 0.4,
        "ink covers {shadows:.2} of the shadows and {highlights:.2} of the highlights"
    );
}

#[test]
fn sharpen_raises_edge_contrast_only_at_edges() {
    let mut stage = Stage::new();
    let (sharpen, insertion) = stage.create(BuiltinEffect::Sharpen);
    let soft = stage.pattern(Pattern::SoftEdge, insertion, 1.0);
    let before = column_profile(&stage.shoot(&stage.frame(vec![soft.clone()], 1, 0.0)));
    let effects = vec![
        soft,
        (
            insertion,
            EffectInstance::new(sharpen).set("sharpness", 1.0),
        ),
    ];
    let after = column_profile(&stage.shoot(&stage.frame(effects, 1, 0.0)));
    let middle = (SIZE / 2) as usize;
    let change = |columns: std::ops::Range<usize>| -> Vec<f32> {
        columns.map(|x| after[x] - before[x]).collect()
    };
    let darkest = change(middle - 6..middle).into_iter().fold(0.0, f32::min);
    let brightest = change(middle..middle + 6).into_iter().fold(0.0, f32::max);
    assert!(
        darkest < -1.5 / 255.0,
        "the dark side darkens by only {darkest}"
    );
    assert!(
        brightest > 1.5 / 255.0,
        "the bright side brightens by only {brightest}"
    );
    let contrast = |profile: &[f32]| profile[middle + 1] - profile[middle - 2];
    assert!(
        contrast(&after) > contrast(&before) + 0.015,
        "edge contrast moves from {} to {}",
        contrast(&before),
        contrast(&after)
    );
    for flat in [16, 240] {
        assert!(
            (after[flat] - before[flat]).abs() <= 1.0 / 255.0,
            "flat column {flat} moved from {} to {}",
            before[flat],
            after[flat]
        );
    }
}

fn darkened(image: &[u8], plain: &[u8], row: u32) -> Vec<u32> {
    (0..SIZE)
        .filter(|x| grey(plain, *x, row) - grey(image, *x, row) > 0.2)
        .collect()
}

#[test]
fn outline_traces_depth_edges_of_selected_layers() {
    let mut stage = Stage::new();
    let (outline, placement) = stage.create(BuiltinEffect::Outline);
    let row = SIZE / 2;
    for antialiasing in [
        Antialiasing::Multisample,
        Antialiasing::Upscale(Upscale::fixed(0.5)),
    ] {
        let plain = stage.shoot_with(&stage.frame(Vec::new(), 2, 0.0), antialiasing);
        let traced = |stage: &mut Stage, mask: u32| {
            let effects = vec![(
                placement,
                EffectInstance::new(outline)
                    .set("mask", mask)
                    .set("width", 6.0),
            )];
            stage.shoot_with(&stage.frame(effects, 2, 0.0), antialiasing)
        };
        let selected = traced(&mut stage, 2);
        let lines = darkened(&selected, &plain, row);
        assert!(
            !lines.is_empty() && lines.len() <= 8,
            "{antialiasing:?}: the near quad's silhouette is traced: {lines:?}"
        );
        assert!(
            lines.iter().all(|x| (16..128).contains(x)),
            "{antialiasing:?}: lines sit on the near quad's edges: {lines:?}"
        );
        for x in [70, 200] {
            assert_eq!(
                rgb(&selected, x, row),
                rgb(&plain, x, row),
                "{antialiasing:?}: column {x} is untouched"
            );
        }
        let other = traced(&mut stage, 1);
        assert!(
            darkened(&other, &plain, row).is_empty(),
            "{antialiasing:?}: a front object outside the mask draws no line"
        );
    }
}

#[test]
fn posterize_snaps_every_channel_to_its_levels() {
    let mut stage = Stage::new();
    let (posterize, insertion) = stage.create(BuiltinEffect::Posterize);
    let ramp = stage.pattern(Pattern::Ramp, insertion, 1.0);
    for dither in ["none", "bayer", "noise"] {
        let effects = vec![
            ramp.clone(),
            (
                insertion,
                EffectInstance::new(posterize)
                    .set("levels", 4)
                    .set("dither.pattern", Value::Enum(dither.into())),
            ),
        ];
        let banded = stage.shoot(&stage.frame(effects, 1, 0.0));
        for y in (0..SIZE).step_by(3) {
            for x in (0..SIZE).step_by(3) {
                for value in rgb(&banded, x, y) {
                    let nearest = (value as f32 / 85.0).round() * 85.0;
                    assert!(
                        (value as f32 - nearest).abs() <= 1.0,
                        "{dither}: {value} at {x},{y} is between levels"
                    );
                }
            }
        }
    }
}

fn red(image: &[u8], x: u32, y: u32) -> i32 {
    rgb(image, x, y)[0]
}

#[test]
fn emission_bloom_glows_only_around_emissive_surfaces() {
    let mut stage = Stage::new();
    let (bloom, placement) = stage.create(BuiltinEffect::EmissionBloom);
    let row = SIZE / 2;
    let (glowing, grey) = (stage.glowing, stage.grey);
    for antialiasing in [
        Antialiasing::Multisample,
        Antialiasing::Upscale(Upscale::fixed(0.5)),
    ] {
        let shoot = |stage: &mut Stage, near: MaterialId, effects: Vec<Placed>| {
            let frame = stage.scene(fixed_look(effects), 1, near, 0.0);
            stage.shoot_with(&frame, antialiasing)
        };
        let plain = shoot(&mut stage, glowing, Vec::new());
        let bloomed = shoot(
            &mut stage,
            glowing,
            vec![(placement, EffectInstance::new(bloom))],
        );
        let beside = 124;
        assert!(
            red(&bloomed, beside, row) > red(&plain, beside, row) + 8,
            "{antialiasing:?}: the wall beside the emissive quad glows: {} vs {}",
            red(&bloomed, beside, row),
            red(&plain, beside, row)
        );
        assert!(
            red(&bloomed, 250, 10) - red(&plain, 250, 10)
                < red(&bloomed, beside, row) - red(&plain, beside, row),
            "{antialiasing:?}: the glow falls off away from the emitter"
        );
        let dull = shoot(&mut stage, grey, Vec::new());
        let dull_bloomed = shoot(
            &mut stage,
            grey,
            vec![(placement, EffectInstance::new(bloom))],
        );
        assert!(
            largest_difference(&dull, &dull_bloomed) <= 1,
            "{antialiasing:?}: bright surfaces without emission do not bloom"
        );
    }
}

#[test]
fn glow_can_take_only_emission() {
    let mut stage = Stage::new();
    let row = SIZE / 2;
    let glowing_look = |source: Option<RenderLayer>, bloom: f32| {
        let mut look = fixed_look(Vec::new());
        let mut composite = Composite::new();
        let exposed = composite.push(Exposure {
            adaptation: 0.0,
            ..Exposure::default()
        });
        let glow = Glow {
            bloom,
            diffusion: 0.0,
            streaks: 0.0,
            flare: 0.0,
            ..Glow::default()
        };
        let image = (Composite::IMAGE, CompositeInput::Node(exposed));
        match source {
            Some(layer) => {
                composite.node(glow, [image, (Glow::SOURCE, CompositeInput::Layer(layer))])
            }
            None => composite.node(glow, [image]),
        };
        composite.push(Grade::default());
        composite.push(Tonemap::default());
        look.composite = composite;
        look
    };
    let emission = Some(RenderLayer::Emission);
    let shoot = |stage: &mut Stage, near: MaterialId, look: Look| {
        let frame = stage.scene(look, 1, near, 0.0);
        stage.shoot(&frame)
    };
    let glowing = stage.glowing;
    let white = flat(&mut stage.renderer, [1.0, 1.0, 1.0, 1.0]);
    let off = shoot(&mut stage, white, glowing_look(emission, 0.0));
    let dull = shoot(&mut stage, white, glowing_look(emission, 1.0));
    assert!(
        largest_difference(&off, &dull) <= 1,
        "emission-only glow leaves surfaces without emission alone"
    );
    let scene_glow = shoot(&mut stage, white, glowing_look(None, 1.0));
    assert!(
        largest_difference(&off, &scene_glow) > 1,
        "scene glow blooms the bright quad"
    );
    let lit = shoot(&mut stage, glowing, glowing_look(emission, 0.0));
    let haloed = shoot(&mut stage, glowing, glowing_look(emission, 1.0));
    assert!(
        red(&haloed, 124, row) > red(&lit, 124, row) + 8,
        "emission blooms past the emitter: {} vs {}",
        red(&haloed, 124, row),
        red(&lit, 124, row)
    );
}

#[test]
fn bright_effects_show_at_defaults_indoors_and_in_daylight() {
    let mut stage = Stage::new();
    for builtin in [BuiltinEffect::LensDirt, BuiltinEffect::LayerBloom] {
        let (id, placement) = stage.create(builtin);
        for daylight in [false, true] {
            let grey = stage.grey;
            let plain = stage.scene(metered_look(daylight, Vec::new()), 1, grey, 0.0);
            let shaded = stage.scene(
                metered_look(daylight, vec![(placement, EffectInstance::new(id))]),
                1,
                grey,
                0.0,
            );
            let before = stage.shoot(&plain);
            let after = stage.shoot(&shaded);
            let difference = mean_difference(&before, &after);
            assert!(
                difference > 0.5,
                "{builtin:?} in {}: the image changes by only {difference:.3}",
                match daylight {
                    true => "daylight",
                    false => "a lamp-lit room",
                }
            );
        }
    }
}
