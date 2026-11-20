use rikka_shader_graph::{
    Arg, Builder, DirectionSpace, Function, FunctionPort, GraphKind, Input, Library, NodeKind,
    Noise, Op, ParameterDecl, PositionSpace, Problem, Ramp, Stop, ValueType, ViewSpace, Wire,
};
use rikka_slang::{Role, TextureKind};

fn problems(result: Result<rikka_shader_graph::Graph, rikka_shader_graph::Error>) -> Vec<Problem> {
    match result {
        Ok(_) => Vec::new(),
        Err(error) => error
            .errors()
            .iter()
            .map(|error| error.problem.clone())
            .collect(),
    }
}

#[test]
fn per_light_values_only_feed_a_lighting_node() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let color = builder.input(Input::LightColor);
    let glow = builder.shader(NodeKind::Emission, [("color", color.into())]);
    builder.output([("surface", glow.into())]);
    assert_eq!(problems(builder.finish()), [Problem::PerLight]);
}

#[test]
fn light_and_surroundings_do_not_combine() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let color = builder.input(Input::LightColor);
    let ambient = builder.input(Input::Ambient);
    let both = builder.add(color, ambient);
    let shader = builder.shader(NodeKind::Lighting, [("light", both.into())]);
    builder.output([("surface", shader.into())]);
    assert!(problems(builder.finish()).contains(&Problem::MixedLighting));
}

#[test]
fn set_sockets_become_uniforms_with_their_value_as_default() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let glow = builder.shader(NodeKind::Emission, [("color", [1.0, 0.5, 0.0].into())]);
    builder.output([("surface", glow.into())]);
    let graph = builder.finish().expect("a valid graph");
    let material = graph.material(&library, "glow").expect("it compiles");

    assert_eq!(material.sockets.len(), 1);
    assert_eq!(material.sockets[0].ty, ValueType::Float3);
    let declared = format!(
        "[Default(1.0, 0.5, 0.0)] [Hint(\"hidden\")] float3 {};",
        material.sockets[0].name
    );
    assert!(material.source.contains(&declared), "{}", material.source);
    assert!(
        !material.source.contains("strength"),
        "the unset strength folds away"
    );
}

#[test]
fn restyling_a_lit_shader_adds_a_lighting_round() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let base = builder.external("bsdf.standard", []);
    let base = builder.out(base, "shader");
    let seen = builder.with(NodeKind::ShaderToRgb, [("shader", base.into())]);
    let seen = builder.out(seen, "color");
    let light = builder.input(Input::LightColor);
    let relit = builder.multiply(light, seen);
    let shader = builder.shader(NodeKind::Lighting, [("light", relit.into())]);
    builder.output([("surface", shader.into())]);
    let graph = builder.finish().expect("a valid graph");
    let source = graph
        .material(&library, "relit")
        .expect("it compiles")
        .source;

    assert_eq!(source.matches("rikka_illuminate(this").count(), 2);
    assert!(source.contains("combine_1(state, context);"));
    assert!(source.contains("if (state.stage == 0u) {"));
}

#[test]
fn unlit_graphs_run_no_light_loop() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let glow = builder.shader(NodeKind::Emission, []);
    builder.output([("surface", glow.into())]);
    let graph = builder.finish().expect("a valid graph");
    let source = graph
        .material(&library, "glow")
        .expect("it compiles")
        .source;
    assert!(!source.contains("rikka_illuminate"));
}

#[test]
fn functions_inline_per_call_into_one_loop_over_the_model() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    builder.function(
        "tinted",
        Function {
            inputs: vec![FunctionPort::new("tint", ValueType::Float3)],
            outputs: vec![FunctionPort::new("shader", ValueType::Shader)],
            ..Function::default()
        },
        |builder| {
            let tint = builder.function_input("tint");
            let lit = builder.external("bsdf.standard", [("base_color", tint.into())]);
            let lit = builder.out(lit, "shader");
            builder.function_output("shader", lit);
        },
    );
    let warm = builder.call("tinted", [("tint", [1.0, 0.5, 0.2].into())]);
    let cool = builder.call("tinted", [("tint", [0.2, 0.5, 1.0].into())]);
    let (warm, cool) = (builder.out(warm, "shader"), builder.out(cool, "shader"));
    let mixed = builder.shader(
        NodeKind::MixShader,
        [
            ("a", warm.into()),
            ("b", cool.into()),
            ("factor", 0.25.into()),
        ],
    );
    builder.output([("surface", mixed.into())]);
    let graph = builder.finish().expect("a valid graph");
    let source = graph
        .material(&library, "mixed")
        .expect("it compiles")
        .source;
    assert_eq!(
        source
            .matches("rikka_closure_lit<RikkaStandardModel>")
            .count(),
        1,
        "both calls share one instance of the model's code\n{source}"
    );
    assert!(source.contains("_shade[2];"), "{source}");
    assert!(!source.contains("rikka_illuminate(this"), "{source}");
}

#[test]
fn displacement_asks_for_a_bound() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let time = builder.input(Input::Time);
    let sway = builder.op(Op::Sine, [time.into()]);
    builder.output([("displacement", Arg::from(sway))]);
    assert_eq!(problems(builder.finish()), [Problem::Unbounded]);

    let mut builder = Builder::new(GraphKind::Material, &library);
    builder.parameter(ParameterDecl::float("reach", 0.1).with_role(Role::Displacement));
    let time = builder.input(Input::Time);
    let sway = builder.op(Op::Sine, [time.into()]);
    builder.output([("displacement", Arg::from(sway))]);
    let graph = builder.finish().expect("a bounded displacement");
    let material = graph.material(&library, "sway").expect("it compiles");
    assert!(material.displaces);
    assert!(material.source.contains("rikka_graph_bounded("));
}

fn fbm(builder: &mut Builder<'_>, scale: f32) -> Wire {
    let position = builder.input(Input::Position(PositionSpace::World));
    let scaled = builder.multiply(position, Arg::from(scale));
    let node = builder.with(
        NodeKind::Fbm(Noise::Gradient),
        [("position", scaled.into())],
    );
    builder.out(node, "value")
}

fn line<'s>(source: &'s str, needle: &str) -> &'s str {
    source
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds {needle}\n{source}"))
}

fn depth(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[test]
fn gates_wrap_only_the_branch_their_operand_owns() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let shared = fbm(&mut builder, 3.0);
    let private = fbm(&mut builder, 5.0);
    let detail = builder.multiply(&private, &shared);
    let mix = builder.parameter(ParameterDecl::float("mix", 0.0));
    let base = builder.lerp([0.2, 0.3, 0.8], detail, mix);
    let glow = builder.add(base, &shared);
    let shader = builder.shader(NodeKind::Emission, [("color", glow.into())]);
    builder.output([("surface", shader.into())]);
    let source = builder
        .finish()
        .expect("a valid graph")
        .material(&library, "gated")
        .expect("it generates")
        .source;

    assert!(
        line(&source, " mix;").starts_with("    [Feature(7)]"),
        "{source}"
    );
    let gate = line(&source, "if (rikka_feature(7u))");
    let owned = line(&source, &format!("_{} = rikka_graph_fbm", private.node()));
    let kept = line(&source, &format!("_{} = rikka_graph_fbm", shared.node()));
    assert_eq!(depth(owned), depth(gate) + 4, "{source}");
    assert_eq!(depth(kept), depth(gate), "{source}");
}

#[test]
fn varying_gates_keep_derivative_reads_outside_their_branch() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let map = builder.parameter(ParameterDecl::texture("map", TextureKind::Colour));
    let uv = builder.input(Input::Uv);
    let read = builder.texture(
        TextureKind::Colour,
        [("texture", map.into()), ("uv", uv.into())],
    );
    let read = builder.out(read, "rgb");
    let mut grain = fbm(&mut builder, 1.0);
    for octave in 2..10 {
        let finer = fbm(&mut builder, octave as f32);
        grain = builder.multiply(grain, finer);
    }
    let detail = builder.multiply(read, grain);
    let normal = builder.input(Input::Normal(DirectionSpace::World));
    let view = builder.input(Input::ViewDirection(ViewSpace::World));
    let facing = builder.dot(normal, view);
    let base = builder.lerp([0.2, 0.3, 0.8], detail, facing);
    let shader = builder.shader(NodeKind::Emission, [("color", base.into())]);
    builder.output([("surface", shader.into())]);
    let source = builder
        .finish()
        .expect("a valid graph")
        .material(&library, "hoisted")
        .expect("it generates")
        .source;

    let gate = line(&source, "if (dot_");
    let sampled = line(&source, ".sampled(");
    let owned = line(&source, "rikka_graph_fbm");
    assert_eq!(depth(sampled), depth(gate), "{source}");
    assert_eq!(depth(owned), depth(gate) + 4, "{source}");
}

#[test]
fn single_stop_ramps_of_light_values_leave_the_light_loop() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let normal = builder.input(Input::Normal(DirectionSpace::World));
    let toward = builder.input(Input::LightDirection);
    let facing = builder.dot(normal, toward);
    let band = builder.ramp(
        Ramp {
            stops: vec![Stop {
                position: 0.0,
                color: [0.4, 0.5, 0.6, 1.0],
            }],
            ..Ramp::default()
        },
        facing,
    );
    let band = builder.swizzle(band, "xyz");
    let tint = builder.parameter(ParameterDecl::color("tint", [1.0, 0.5, 0.2]));
    let painted = builder.multiply(band, tint);
    let color = builder.input(Input::LightColor);
    let lit = builder.multiply(color, painted);
    let shader = builder.shader(NodeKind::Lighting, [("light", lit.into())]);
    builder.output([("surface", shader.into())]);
    let source = builder
        .finish()
        .expect("a valid graph")
        .material(&library, "banded")
        .expect("it generates")
        .source;

    let direct = source
        .split("void direct(")
        .nth(1)
        .and_then(|rest| rest.split("void indirect(").next())
        .expect("a light loop");
    assert!(!direct.contains("dot("), "{direct}");
    assert!(!direct.contains("tint"), "{direct}");
}

#[test]
fn mix_factors_skip_the_light_loop_of_the_dead_closure() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let glossy = builder.external("bsdf.principled", [("roughness", 0.4.into())]);
    let glossy = builder.out(glossy, "shader");
    let rough = builder.external("bsdf.diffuse", []);
    let rough = builder.out(rough, "shader");
    let factor = builder.parameter(ParameterDecl::float("factor", 0.0));
    let mixed = builder.shader(
        NodeKind::MixShader,
        [
            ("a", glossy.into()),
            ("b", rough.into()),
            ("factor", factor.into()),
        ],
    );
    builder.output([("surface", mixed.into())]);
    let source = builder
        .finish()
        .expect("a valid graph")
        .material(&library, "mixed")
        .expect("it generates")
        .source;

    for (model, live) in [
        ("RikkaOpenPbrModel", " >= 1.0), context);"),
        ("RikkaDiffuseModel", " <= 0.0), context);"),
    ] {
        let call = line(&source, &format!("rikka_closure_lit<{model}>"));
        assert!(call.contains(live), "{source}");
    }
}
