use rikka_renderer::{Node, Renderer, Scene};
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec2, Vec3, Vec4};
use rikka_renderer_3d::{
    Background, BuiltinEffect, Composite, CompositeInput, Draw, Effect, EffectId, EffectInstance,
    EffectOutput, EffectStep, EffectUniform, Environment, Frame, Look, MaterialId, MeshId,
    Operation, RenderLayer, ShaderCompiler, Tone, Tonemap, Value, Viewport, ViewportConfig,
};
use rikka_types::{Color, Rect};

use crate::support::{ambient_frame, flat, quad, renderer};

pub(crate) const SIZE: u32 = 256;
pub(crate) const CENTER: i32 = SIZE as i32 / 2;

const PATTERN: &str = r#"
import rikka;

struct Parameters {
    [Options("flat|impulse|square|disc|step|card|noisy_step")] uint shape;
    [Default(1.0, 1.0, 1.0, 1.0)] float4 color;
    [Default(0.0, 0.0, 0.0, 1.0)] float4 background;
    [Default(1.0)] float extent;
    float2 offset;
};

struct Push {
    Parameters* parameters;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void paint(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters p = *push.parameters;
    int2 center = int2(push.size / 2u) + int2(p.offset);
    float2 away = float2(pixel - center);
    float4 color = p.background;
    switch (p.shape) {
    case 0u:
        color = p.color;
        break;
    case 1u:
        color = all(pixel == center) ? p.color : p.background;
        break;
    case 2u:
        color = all(abs(away + 0.5) < 0.5 * p.extent) ? p.color : p.background;
        break;
    case 3u:
        color = length(away) <= p.extent ? p.color : p.background;
        break;
    case 4u:
        color = pixel.x >= center.x ? p.color : p.background;
        break;
    case 6u: {
        float noise = rikka_effect_random(uint3(uint(pixel.x), uint(pixel.y), 33u)) - 0.5;
        color = (pixel.x >= center.x ? p.color : p.background) + float4(float3(noise * 0.1), 0.0);
        break;
    }
    default: {
        float2 unit = float2(pixel) / float2(push.size - 1u);
        bool checker = ((pixel.x / 16) + (pixel.y / 16)) % 2 == 0;
        float noise = rikka_effect_random(uint3(uint(pixel.x), uint(pixel.y), 77u)) - 0.5;
        color = float4(float3(unit.x, unit.y, 1.0 - unit.x) * (checker ? 1.0 : 0.45) + noise * 0.05, 1.0) * p.color;
        break;
    }
    }
    rikka_effect_store(push.target, pixel, max(color, float4(0.0)));
}
"#;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Shape {
    Flat,
    Impulse,
    Square,
    Disc,
    Step,
    Card,
    NoisyStep,
}

impl Shape {
    fn option(self) -> &'static str {
        match self {
            Shape::Flat => "flat",
            Shape::Impulse => "impulse",
            Shape::Square => "square",
            Shape::Disc => "disc",
            Shape::Step => "step",
            Shape::Card => "card",
            Shape::NoisyStep => "noisy_step",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Paint {
    pub shape: Shape,
    pub color: Vec4,
    pub background: Vec4,
    pub extent: f32,
    pub offset: Vec2,
}

impl Paint {
    pub(crate) fn new(shape: Shape, color: Vec3) -> Paint {
        Paint {
            shape,
            color: color.extend(1.0),
            background: Vec4::new(0.0, 0.0, 0.0, 1.0),
            extent: 1.0,
            offset: Vec2::ZERO,
        }
    }

    pub(crate) fn extent(mut self, extent: f32) -> Paint {
        self.extent = extent;
        self
    }

    pub(crate) fn background(mut self, background: Vec3) -> Paint {
        self.background = background.extend(1.0);
        self
    }

    pub(crate) fn offset(mut self, offset: Vec2) -> Paint {
        self.offset = offset;
        self
    }
}

pub(crate) struct Shot(Vec<u8>);

impl Shot {
    pub(crate) fn rgb(&self, x: i32, y: i32) -> [u8; 3] {
        let at = ((y as u32 * SIZE + x as u32) * 4) as usize;
        [self.0[at], self.0[at + 1], self.0[at + 2]]
    }

    pub(crate) fn linear(&self, x: i32, y: i32) -> Vec3 {
        let [r, g, b] = self.rgb(x, y);
        Vec3::new(decode(r), decode(g), decode(b))
    }

    pub(crate) fn largest_difference(&self, other: &Shot) -> u8 {
        self.0
            .iter()
            .zip(&other.0)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0)
    }
}

pub(crate) fn decode(value: u8) -> f32 {
    let encoded = f32::from(value) / 255.0;
    match encoded <= 0.04045 {
        true => encoded / 12.92,
        false => ((encoded + 0.055) / 1.055).powf(2.4),
    }
}

pub(crate) fn encode(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    255.0
        * match value <= 0.003_130_8 {
            true => value * 12.92,
            false => 1.055 * value.powf(1.0 / 2.4) - 0.055,
        }
}

pub(crate) struct Canvas {
    renderer: rikka_test_renderer::TestRenderer,
    pattern: EffectId,
    wall: MeshId,
    grey: MaterialId,
}

impl Canvas {
    pub(crate) fn new() -> Canvas {
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
        let wall = quad(&mut renderer, 0.0, 4.0);
        let grey = flat(&mut renderer, [0.5, 0.5, 0.5, 1.0]);
        Canvas {
            renderer,
            pattern,
            wall,
            grey,
        }
    }

    pub(crate) fn effect(&mut self, builtin: BuiltinEffect) -> EffectId {
        self.renderer
            .three()
            .create_effect(&builtin.effect())
            .expect("built-in effects are valid")
    }

    pub(crate) fn paint(&self, paint: Paint) -> EffectInstance {
        EffectInstance::new(self.pattern)
            .set("shape", Value::Enum(paint.shape.option().into()))
            .set("color", Value::Vec4(paint.color))
            .set("background", Value::Vec4(paint.background))
            .set("extent", paint.extent)
            .set("offset", Value::Vec2(paint.offset))
    }

    pub(crate) fn viewport(&mut self) -> Viewport {
        self.renderer
            .three()
            .create_viewport(ViewportConfig::new(SIZE, SIZE))
            .expect("a viewport")
    }

    pub(crate) fn shoot(&mut self, composite: Composite) -> Shot {
        let mut viewport = self.viewport();
        self.shoot_on(&mut viewport, composite, 0.0)
    }

    pub(crate) fn shoot_on(
        &mut self,
        viewport: &mut Viewport,
        composite: Composite,
        time: f32,
    ) -> Shot {
        let mut look: Look = rikka_test_renderer_3d::minimal_look();
        look.environment = Environment::None;
        look.composite = composite;
        let mut frame: Frame = ambient_frame(0.4);
        frame
            .background(Background::Color(Vec3::ZERO))
            .time(time)
            .look(look);
        frame.draw(Draw::new(self.wall, self.grey));
        let renderer: &mut Renderer = &mut self.renderer;
        renderer
            .three()
            .render(viewport, &frame)
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
        Shot(renderer.read_pixels(&mut target).expect("pixels come back"))
    }
}

pub(crate) fn chain(operations: impl IntoIterator<Item = Operation>) -> Composite {
    let mut composite = Composite::new();
    for operation in operations {
        composite.push(operation);
    }
    composite.push(Tonemap::new(Tone::Clamp));
    composite
}

pub(crate) fn sized(
    painted: EffectInstance,
    mask: EffectInstance,
    effect: EffectInstance,
    socket: &str,
) -> Composite {
    let mut composite = Composite::new();
    let image = composite.push(painted);
    let size = composite.node(
        mask,
        [(Composite::IMAGE, CompositeInput::Layer(RenderLayer::Image))],
    );
    let filtered = composite.node(
        effect,
        [
            (Composite::IMAGE, CompositeInput::Node(image)),
            (socket, CompositeInput::Node(size)),
        ],
    );
    let mapped = composite.node(
        Tonemap::new(Tone::Clamp),
        [(Composite::IMAGE, CompositeInput::Node(filtered))],
    );
    composite.output(CompositeInput::Node(mapped));
    composite
}
