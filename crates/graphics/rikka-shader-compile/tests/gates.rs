use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{MaterialDesc, MaterialId, Three};
use rikka_shader_compile::Material;
use rikka_shader_graph::{
    Arg, Builder, DirectionSpace, Function, FunctionPort, Graph, GraphKind, Input, Library,
    NodeKind, Noise, Op, ParameterDecl, PositionSpace, ValueType, Wire,
};

use crate::support::{SIZE, compiled, compiler, lit, material, pixels, renderer, rgb, sphere};

const LIGHT: Vec3 = Vec3::new(-1.0, -0.3, -1.0);

const NEARLY_CLOSED: f32 = 1e-30;

const CONTROLS: [&str; 7] = [
    "t_surface",
    "amount",
    "t_light",
    "t_ambient",
    "t_post",
    "weight",
    "factor",
];

fn noise(builder: &mut Builder<'_>, scale: f32) -> Wire {
    let position = builder.input(Input::Position(PositionSpace::World));
    let scaled = builder.multiply(position, Arg::from(scale));
    let node = builder.with(
        NodeKind::Fbm(Noise::Gradient),
        [("position", scaled.into())],
    );
    builder.out(node, "value")
}

fn knob(builder: &mut Builder<'_>, name: &str) -> Wire {
    builder.parameter(ParameterDecl::float(name, 0.0))
}

fn gated(library: &Library) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    builder.function(
        "grade",
        Function {
            inputs: vec![FunctionPort::new("color", ValueType::Float3)],
            outputs: vec![FunctionPort::new("color", ValueType::Float3)],
            ..Function::default()
        },
        |builder| {
            let color = builder.function_input("color");
            let amount = knob(builder, "amount");
            let shade = builder.external(
                "color.saturated_shade",
                [("color", color.into()), ("amount", amount.into())],
            );
            let shade = builder.out(shade, "value");
            builder.function_output("color", shade);
        },
    );
    let shared = noise(&mut builder, 3.0);
    let private = noise(&mut builder, 5.0);
    let detail = builder.multiply(private, &shared);
    let t_surface = knob(&mut builder, "t_surface");
    let base = builder.op(
        Op::Lerp,
        [[0.2, 0.3, 0.8].into(), detail.into(), t_surface.into()],
    );
    let graded = builder.call("grade", [("color", base.into())]);
    let graded = builder.out(graded, "color");

    let normal = builder.input(Input::Normal(DirectionSpace::World));
    let toward = builder.input(Input::LightDirection);
    let facing = builder.dot(normal, toward);
    let light_detail = noise(&mut builder, 7.0);
    let t_light = knob(&mut builder, "t_light");
    let shaped = builder.op(
        Op::Lerp,
        [facing.into(), light_detail.into(), t_light.into()],
    );
    let shaped = builder.op(Op::Saturate, [shaped.into()]);
    let color = builder.input(Input::LightColor);
    let lit = builder.multiply(color, shaped);
    let lit = builder.multiply(lit, &graded);

    let ambient_detail = noise(&mut builder, 9.0);
    let t_ambient = knob(&mut builder, "t_ambient");
    let occluded = builder.op(
        Op::Lerp,
        [1.0.into(), ambient_detail.into(), t_ambient.into()],
    );
    let ambient = builder.input(Input::Ambient);
    let ambient = builder.multiply(ambient, occluded);
    let ambient = builder.multiply(ambient, &graded);
    let lighting = builder.shader(
        NodeKind::Lighting,
        [("light", lit.into()), ("ambient", ambient.into())],
    );

    let seen = builder.with(NodeKind::ShaderToRgb, [("shader", lighting.into())]);
    let seen = builder.out(seen, "color");
    let post_detail = noise(&mut builder, 11.0);
    let t_post = knob(&mut builder, "t_post");
    let toned = builder.op(Op::Lerp, [seen.into(), post_detail.into(), t_post.into()]);
    let glow_detail = noise(&mut builder, 13.0);
    let switched = builder.op(Op::Switch, [glow_detail.into(), 0.25.into(), 1.0.into()]);
    let weight = knob(&mut builder, "weight");
    let glow = builder.multiply(switched, weight);
    let glow = builder.multiply(glow, &shared);
    let glowing = builder.add(toned, glow);
    let emission = builder.shader(NodeKind::Emission, [("color", glowing.into())]);

    let tint = builder.multiply([0.8, 0.3, 0.2], &shared);
    let glossy = builder.external(
        "bsdf.principled",
        [("base_color", tint.into()), ("roughness", 0.4.into())],
    );
    let glossy = builder.out(glossy, "shader");
    let rough = builder.external("bsdf.diffuse", [("color", [0.3, 0.6, 0.2].into())]);
    let rough = builder.out(rough, "shader");
    let factor = knob(&mut builder, "factor");
    let mixed = builder.shader(
        NodeKind::MixShader,
        [
            ("a", glossy.into()),
            ("b", rough.into()),
            ("factor", factor.into()),
        ],
    );
    let surface = builder.shader(
        NodeKind::AddShader,
        [("a", mixed.into()), ("b", emission.into())],
    );
    builder.output([("surface", surface.into())]);
    builder.finish().expect("a valid graph")
}

fn drawn(renderer: &mut Renderer, graphed: &Material, value: f32) -> Vec<u8> {
    let mut desc = MaterialDesc::new(graphed.template());
    for name in CONTROLS {
        desc = desc.with(name, value).expect("the control is a parameter");
    }
    let id: MaterialId = material(renderer, desc);
    let mesh = sphere(renderer);
    let shown = pixels(renderer, &lit(mesh, &[id], LIGHT));
    renderer
        .three()
        .destroy_material(id)
        .expect("a live material");
    shown
}

#[test]
fn gated_branches_draw_exactly_like_their_evaluated_branches() {
    let mut renderer = renderer();
    let library = Library::standard();
    let graph = gated(&library);
    let source = graph
        .material(&library, "gated")
        .expect("it generates")
        .source;
    assert!(
        source.matches("if (").count() >= 7,
        "every gate kind wraps its branch\n{source}"
    );
    let graphed = compiled(&graph, &library, &compiler());
    let skipped = drawn(&mut renderer, &graphed, 0.0);
    let evaluated = drawn(&mut renderer, &graphed, NEARLY_CLOSED);
    assert!(rgb(&skipped, SIZE / 2, SIZE / 2) != [0, 0, 0]);
    let differing = (0..SIZE * SIZE)
        .map(|at| (at % SIZE, at / SIZE))
        .find(|(x, y)| rgb(&skipped, *x, *y) != rgb(&evaluated, *x, *y));
    if let Some((x, y)) = differing {
        panic!(
            "skipped branches change {x},{y}: {:?} against {:?}",
            rgb(&skipped, x, y),
            rgb(&evaluated, x, y)
        );
    }
    let open = drawn(&mut renderer, &graphed, 0.5);
    assert!(skipped != open, "the gates open at other values");
}
