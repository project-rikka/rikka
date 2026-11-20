use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    BuiltinTemplate, Cull, MaterialDesc, MaterialState, MaterialTemplate, TemplateReadiness, Value,
};
use rikka_shader_graph::{
    Arg, Builder, Constant, Curve, GraphKind, Input, Interpolation, Library, NodeKind, Op, Scope,
    SocketRef, SocketValue,
};

use crate::support::{
    SIZE, built, compiled, compiler, lit, material, pixels, renderer, rgb, sphere, sphere_of,
};

const LIGHT: Vec3 = Vec3::new(-1.0, -0.3, -1.0);

fn close(a: &[u8], b: &[u8], tolerance: i32) -> bool {
    a.iter()
        .zip(b)
        .all(|(a, b)| (i32::from(*a) - i32::from(*b)).abs() <= tolerance)
}

fn drawable(template: &MaterialTemplate) -> bool {
    matches!(template.readiness(), TemplateReadiness::Drawable)
}

#[test]
fn principled_closure_draws_like_the_standard_template_in_both_builds() {
    let mut renderer = renderer();
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let surface = builder.external(
        "bsdf.principled",
        [
            ("base_color", [0.8, 0.3, 0.2].into()),
            ("roughness", 0.4.into()),
        ],
    );
    let shader = builder.out(surface, "shader");
    builder.output([("surface", shader.into())]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let template = graphed.template();
    let from_graph = material(&mut renderer, MaterialDesc::new(template));
    let mesh = sphere(&mut renderer);
    let frame = lit(mesh, &[from_graph], LIGHT);
    let drafted = pixels(&mut renderer, &frame);
    let draft = template.generation();
    let optimized = built(&mut renderer, &frame, template);
    assert!(
        template.generation() > draft,
        "the optimized build replaces the draft"
    );

    let standard = MaterialTemplate::builtin(BuiltinTemplate::Standard);
    let desc = MaterialDesc::new(&standard)
        .with("base_color", Vec3::new(0.8, 0.3, 0.2))
        .and_then(|desc| desc.with("specular_roughness", 0.4))
        .expect("standard parameters");
    let reference = material(&mut renderer, desc);
    let reference = sphere_of(&mut renderer, &[reference], LIGHT);

    assert!(rgb(&drafted, SIZE / 2, SIZE / 2) != [0, 0, 0]);
    assert!(close(&drafted, &reference, 2), "the draft matches");
    assert!(
        close(&optimized, &reference, 2),
        "the optimized build matches"
    );
}

fn cel(library: &Library) -> Builder<'_> {
    let mut builder = Builder::new(GraphKind::Material, library);
    let normal = builder.input(Input::Normal(rikka_shader_graph::DirectionSpace::World));
    let toward = builder.input(Input::LightDirection);
    let facing = builder.dot(normal, toward);
    let band = builder.external(
        "npr.toon_step",
        [
            ("value", facing.into()),
            ("edge", 0.0.into()),
            ("feather", 0.0.into()),
        ],
    );
    let band = builder.out(band, "value");
    let color = builder.input(Input::LightColor);
    let lit = builder.multiply(color, band);
    let tinted = builder.multiply(lit, Arg::from([0.5, 0.25, 0.125]));
    let shader = builder.shader(NodeKind::Lighting, [("light", tinted.into())]);
    builder.output([("surface", shader.into())]);
    builder
}

#[test]
fn custom_lighting_runs_its_graph_once_per_light() {
    let mut renderer = renderer();
    let library = Library::standard();
    let graph = cel(&library).finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let pixels = sphere_of(&mut renderer, &[drawn], Vec3::new(-1.0, 0.0, -0.2));

    let lit = rgb(&pixels, SIZE * 3 / 4, SIZE / 2);
    let dark = rgb(&pixels, SIZE / 4, SIZE / 2);
    assert_eq!(
        lit,
        [255, 188, 137],
        "two units of light times the tint, clamped and encoded"
    );
    assert_eq!(dark, [0, 0, 0], "the cel edge leaves the far side unlit");
}

#[test]
fn shader_to_colour_feeds_a_second_round_of_lighting() {
    let mut renderer = renderer();
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let base = builder.external("bsdf.principled", [("base_color", [1.0, 1.0, 1.0].into())]);
    let base = builder.out(base, "shader");
    let seen = builder.with(NodeKind::ShaderToRgb, [("shader", base.into())]);
    let seen = builder.out(seen, "color");
    let brightness = builder.op(Op::Luminance, [seen.into()]);
    let banded = builder.curve(
        Curve {
            points: vec![[0.0, 0.0], [0.3, 1.0]],
            interpolation: Interpolation::Constant,
        },
        brightness,
    );
    let color = builder.input(Input::LightColor);
    let relit = builder.multiply(color, banded);
    let shader = builder.shader(NodeKind::Lighting, [("light", relit.into())]);
    builder.output([("surface", shader.into())]);
    let graph = builder.finish().expect("a valid graph");
    let source = graph
        .material(&library, "graph")
        .expect("it compiles")
        .source;
    assert_eq!(source.matches("rikka_illuminate(this").count(), 2);

    let graphed = compiled(&graph, &library, &compiler());
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let pixels = sphere_of(&mut renderer, &[drawn], Vec3::new(-1.0, 0.0, -0.2));
    let mut seen: Vec<[i32; 3]> = (0..SIZE).map(|x| rgb(&pixels, x, SIZE / 2)).collect();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(
        seen,
        vec![[0, 0, 0], [255, 255, 255]],
        "the banded relight is two-tone"
    );
}

#[test]
fn set_sockets_change_live_and_unset_ones_fold() {
    let mut renderer = renderer();
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let glow = builder.node(NodeKind::Emission);
    builder.set(glow, "color", [1.0, 0.0, 0.0]);
    let shader = builder.out(glow, "shader");
    builder.output([("surface", shader.into())]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let names: Vec<&str> = graphed
        .sockets()
        .iter()
        .map(|socket| socket.at.port.as_str())
        .collect();
    assert_eq!(
        names,
        ["color"],
        "only the set socket is a uniform; strength folds"
    );

    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let red = sphere_of(&mut renderer, &[drawn], LIGHT);
    assert_eq!(rgb(&red, SIZE / 2, SIZE / 2), [255, 0, 0]);

    let recoloured: Vec<(&str, Value)> = graphed
        .values(
            |at: &SocketRef| {
                (*at == SocketRef::new(Scope::Main, glow, "color"))
                    .then(|| SocketValue::Number(Constant::vec3(0.0, 0.0, 1.0)))
            },
            |_| None,
        )
        .collect();
    renderer
        .three()
        .set_material_values(drawn, &recoloured)
        .expect("the uniform takes the value");
    let blue = sphere_of(&mut renderer, &[drawn], LIGHT);
    assert_eq!(rgb(&blue, SIZE / 2, SIZE / 2), [0, 0, 255]);
}

#[test]
fn outline_shader_draws_the_hull_around_the_body() {
    let mut renderer = renderer();
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let body = builder.shader(NodeKind::Emission, [("color", [0.0, 1.0, 0.0].into())]);
    let line = builder.shader(NodeKind::Emission, [("color", [1.0, 0.0, 0.0].into())]);
    builder.output([
        ("surface", body.into()),
        ("outline", line.into()),
        ("outline_width", 0.15.into()),
    ]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    assert!(graphed.source().outlined);
    let template = graphed.template();
    let mesh = sphere(&mut renderer);
    let body = material(&mut renderer, MaterialDesc::new(template));
    built(&mut renderer, &lit(mesh, &[body], LIGHT), template);
    let hull = material(
        &mut renderer,
        MaterialDesc::new(template)
            .state(MaterialState::opaque().cull(Cull::Front))
            .hull(),
    );
    assert!(
        drawable(template),
        "the hull variants wait for their optimized build"
    );
    let frame = lit(mesh, &[body, hull], LIGHT);
    let drafted = pixels(&mut renderer, &frame);
    let optimized = built(&mut renderer, &frame, template);

    for drawn in [drafted, optimized] {
        assert_eq!(rgb(&drawn, SIZE / 2, SIZE / 2), [0, 255, 0]);
        let ring = (0..SIZE / 2)
            .map(|x| rgb(&drawn, x, SIZE / 2))
            .find(|color| *color != [0, 0, 0])
            .expect("the sphere is on screen");
        assert_eq!(ring, [255, 0, 0], "the silhouette starts with the outline");
    }
}

#[test]
fn subsurface_routing_optimizes_the_scatter_variant_when_it_appears() {
    let mut renderer = renderer();
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let surface = builder.external(
        "bsdf.principled",
        [
            ("base_color", [0.8, 0.5, 0.4].into()),
            ("subsurface_weight", 1.0.into()),
        ],
    );
    let shader = builder.out(surface, "shader");
    builder.output([("surface", shader.into())]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let weight = graphed
        .sockets()
        .iter()
        .find(|socket| socket.at.port == "subsurface_weight")
        .map(|socket| socket.name.clone())
        .expect("the set weight is a socket");
    let template = graphed.template();
    let skin = material(
        &mut renderer,
        MaterialDesc::new(template)
            .with(&weight, 0.0)
            .expect("the weight is a parameter"),
    );
    let mesh = sphere(&mut renderer);
    let mut frame = lit(mesh, &[skin], LIGHT);
    frame.look(rikka_test_renderer_3d::minimal_look());
    built(&mut renderer, &frame, template);

    renderer
        .three()
        .set_material(skin, &weight, 1.0)
        .expect("the weight takes the value");
    let scattered = pixels(&mut renderer, &frame);
    assert!(
        drawable(template),
        "the scatter variant waits for its optimized build"
    );
    let optimized = built(&mut renderer, &frame, template);
    assert!(rgb(&scattered, SIZE / 2, SIZE / 2) != [0, 0, 0]);
    assert!(
        close(&scattered, &optimized, 2),
        "both builds scatter alike"
    );
}
