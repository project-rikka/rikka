use std::collections::HashMap;

use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    AgxLook, Background, BuiltinEffect, CompositeInput, EffectId, Environment, Frame, Look,
    Metering, Operation, RenderLayer, ShaderCompiler, TextureBinding, Three, Tone,
};
use rikka_shader_compile::{Resolve, composite, filters, library, pixel_effect};
use rikka_shader_graph::{
    Builder, Category, CompositePlan, Env, Graph, GraphKind, Layer, Library, PixelStep,
    PortDefault, PortType, SocketRef, SocketValue, ValueType,
};

use crate::support::{SIZE, compiler, frame, pixels, renderer, rgb};

struct Effects<'a> {
    renderer: &'a mut Renderer,
    compiler: Option<&'a ShaderCompiler>,
    graph: &'a Graph,
    pixels: HashMap<u64, EffectId>,
}

impl Resolve for Effects<'_> {
    fn pixel(&mut self, step: &PixelStep) -> Option<EffectId> {
        let compiler = self.compiler?;
        if let Some(id) = self.pixels.get(&step.key) {
            return Some(*id);
        }
        let effect = pixel_effect(step, compiler, "graph.pixel")
            .unwrap_or_else(|error| panic!("{error}\n{}", step.source));
        let id = self
            .renderer
            .three()
            .create_effect(&effect)
            .expect("the pass is a valid effect");
        self.pixels.insert(step.key, id);
        Some(id)
    }

    fn builtin(&mut self, effect: BuiltinEffect) -> Option<EffectId> {
        self.renderer.three().create_effect(&effect.effect()).ok()
    }

    fn socket(&mut self, at: &SocketRef) -> Option<SocketValue> {
        self.graph.socket(at).cloned()
    }

    fn texture(&mut self, _: &str) -> Option<TextureBinding> {
        None
    }
}

fn planned(graph: &Graph, library: &Library) -> CompositePlan {
    graph.composite(library).expect("the composite plans")
}

fn look_with(
    renderer: &mut Renderer,
    compiler: Option<&ShaderCompiler>,
    graph: &Graph,
    plan: &CompositePlan,
) -> Look {
    let records = plan.evaluate(&Env {
        time: 0.0,
        sockets: &|at: &SocketRef| graph.socket(at).cloned(),
    });
    let mut effects = Effects {
        renderer,
        compiler,
        graph,
        pixels: HashMap::new(),
    };
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.environment = Environment::None;
    look.composite = composite(plan, &records, &mut effects).expect("the composite maps");
    look
}

fn grey(look: Look) -> Frame {
    let mut frame = frame();
    frame
        .background(Background::Color(Vec3::splat(0.5)))
        .ambient(Vec3::ZERO)
        .look(look);
    frame
}

fn tinted(library: &Library) -> Graph {
    let mut builder = Builder::new(GraphKind::Composite, library);
    let scene = builder.layer(Layer::Image);
    let green = builder.multiply(scene, [0.0, 1.0, 0.0, 1.0]);
    builder.output([("image", green.into())]);
    builder.finish().expect("a valid composite")
}

#[test]
fn pixel_maths_runs_as_a_generated_pass() {
    let mut renderer = renderer();
    let library = library().expect("the filter library builds");
    let graph = tinted(&library);
    let plan = planned(&graph, &library);
    let compiler = compiler();
    let look = look_with(&mut renderer, Some(&compiler), &graph, &plan);
    assert_eq!(look.composite.nodes.len(), 1);

    let shown = pixels(&mut renderer, &grey(look));
    let [red, green, blue] = rgb(&shown, SIZE / 2, SIZE / 2);
    assert!(red == 0 && blue == 0 && green > 100, "{red} {green} {blue}");
}

#[test]
fn passes_still_compiling_let_the_image_through() {
    let mut renderer = renderer();
    let library = library().expect("the filter library builds");
    let graph = tinted(&library);
    let plan = planned(&graph, &library);
    let look = look_with(&mut renderer, None, &graph, &plan);

    assert!(look.composite.nodes.is_empty());
    assert_eq!(
        look.composite.output,
        CompositeInput::Layer(RenderLayer::Image)
    );
}

#[test]
fn filters_become_native_operations_and_built_in_effects() {
    let mut renderer = renderer();
    let library = library().expect("the filter library builds");
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let mut image = builder.layer(Layer::Image);
    for (id, port, value) in [
        ("filter.exposure", "metering", 1.0),
        ("filter.glow", "bloom", 0.3),
        ("filter.grade", "saturation", 0.0),
        ("filter.tonemap", "tone", 4.0),
        ("effect.pixelate", "", 0.0),
        ("filter.lens", "vignette", 0.0),
    ] {
        let node = builder.external(id, [("image", image.into())]);
        if !port.is_empty() {
            builder.set(node, port, value);
        }
        image = builder.out(node, "image");
    }
    builder.output([("image", image.into())]);
    let graph = builder.finish().expect("a valid composite");
    let plan = planned(&graph, &library);
    let look = look_with(&mut renderer, None, &graph, &plan);

    let kinds: Vec<&str> = look
        .composite
        .nodes
        .iter()
        .map(|node| match &node.operation {
            Operation::Exposure(_) => "exposure",
            Operation::MotionBlur(_) => "motion blur",
            Operation::DepthOfField(_) => "depth of field",
            Operation::Glow(_) => "glow",
            Operation::Grade(_) => "grade",
            Operation::Tonemap(_) => "tonemap",
            Operation::Lens(_) => "lens",
            Operation::Effect(_) => "effect",
        })
        .collect();
    assert_eq!(
        kinds,
        ["exposure", "glow", "grade", "tonemap", "effect", "lens"]
    );

    let shown = pixels(&mut renderer, &grey(look));
    let [red, green, blue] = rgb(&shown, SIZE / 2, SIZE / 2);
    assert!(
        red == green && green == blue && red > 0,
        "{red} {green} {blue}"
    );
}

#[test]
fn every_built_in_effect_node_describes_itself_and_its_image_sockets() {
    let specs = filters().expect("the filters build");
    for builtin in BuiltinEffect::ALL {
        let id = format!("effect.{}", builtin.name());
        let spec = specs
            .iter()
            .find(|spec| spec.id == id)
            .unwrap_or_else(|| panic!("{id} has no node"));
        assert_eq!(spec.title, builtin.title());
        assert_eq!(spec.doc, builtin.doc());
        let images: Vec<(&str, PortDefault)> = spec
            .inputs
            .iter()
            .filter(|port| port.ty == PortType::Value(ValueType::Image))
            .map(|port| (port.name, port.default))
            .collect();
        let effect = builtin.effect();
        let expected: Vec<(&str, PortDefault)> = effect
            .sockets()
            .map(|(name, wiring)| {
                let default = match (name, wiring) {
                    ("image", _) | (_, rikka_renderer_3d::Wiring::Required) => {
                        PortDefault::Required
                    }
                    (_, rikka_renderer_3d::Wiring::Optional) => PortDefault::Empty,
                };
                (name, default)
            })
            .collect();
        assert_eq!(images, expected, "{id}");
    }
    let bokeh = specs
        .iter()
        .find(|spec| spec.id == "effect.bokeh_blur")
        .expect("a bokeh node");
    assert_eq!(bokeh.category, Category::Filter);
    for socket in ["size", "aperture"] {
        assert!(
            bokeh.inputs.iter().any(|port| port.name == socket
                && port.default == PortDefault::Empty
                && port.ty == PortType::Value(ValueType::Image)),
            "bokeh blur lacks an optional {socket} image"
        );
    }
}

#[test]
fn new_filters_map_to_operations_with_their_sockets() {
    let mut renderer = renderer();
    let library = library().expect("the filter library builds");
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let image = builder.layer(Layer::Image);
    let depth = builder.layer(Layer::Depth);
    let exposure = builder.external(
        "filter.exposure",
        [
            ("image", image.into()),
            ("metering", 2.0.into()),
            ("aperture", 4.0.into()),
        ],
    );
    let exposed = builder.out(exposure, "image");
    let blur = builder.external(
        "effect.gaussian_blur",
        [("image", exposed.into()), ("size", depth.into())],
    );
    let blurred = builder.out(blur, "image");
    let tonemap = builder.external(
        "filter.tonemap",
        [
            ("image", blurred.into()),
            ("tone", 5.0.into()),
            ("look", 2.0.into()),
            ("preserve_hue", 0.5.into()),
        ],
    );
    let mapped = builder.out(tonemap, "image");
    builder.output([("image", mapped.into())]);
    let graph = builder.finish().expect("a valid composite");
    let plan = planned(&graph, &library);
    let look = look_with(&mut renderer, None, &graph, &plan);

    let nodes = &look.composite.nodes;
    assert_eq!(nodes.len(), 3);
    match &nodes[0].operation {
        Operation::Exposure(exposure) => {
            assert_eq!(exposure.metering, Metering::Physical);
            assert_eq!(exposure.aperture, 4.0);
        }
        other => panic!("{other:?} instead of an exposure"),
    }
    assert!(matches!(nodes[1].operation, Operation::Effect(_)));
    assert_eq!(
        nodes[1].input("size"),
        Some(CompositeInput::Layer(RenderLayer::Depth))
    );
    assert_eq!(nodes[1].input("image"), Some(CompositeInput::Node(0)));
    match &nodes[2].operation {
        Operation::Tonemap(tonemap) => {
            assert_eq!(tonemap.tone, Tone::Hable);
            assert_eq!(tonemap.look, AgxLook::Golden);
            assert_eq!(tonemap.preserve_hue, 0.5);
        }
        other => panic!("{other:?} instead of a tone map"),
    }
}
