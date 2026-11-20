use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Cull, Draw, Figure, Head, Light, MaterialDesc, MaterialId, MaterialState, ShadowQuality,
    TextureBinding, Value,
};
use rikka_shader_compile::Material;
use rikka_shader_graph::{
    Arg, Builder, Constant, GraphKind, Input, Library, NodeKind, Op, SocketRef, SocketValue, Wire,
};

use crate::support::{
    SCREEN_HALF, SIZE, compiled, compiler, frame, looked, material, near, pixels, quad, renderer,
    rgb, sphere_of, texture,
};

fn lighting(builder: &mut Builder<'_>, value: Wire) {
    let color = builder.input(Input::LightColor);
    let lit = builder.multiply(color, value);
    let lit = builder.multiply(lit, 0.5);
    let shader = builder.shader(NodeKind::Lighting, [("light", lit.into())]);
    builder.output([("surface", shader.into())]);
}

fn emitting(builder: &mut Builder<'_>, value: Wire) {
    let glow = builder.shader(NodeKind::Emission, [("color", value.into())]);
    builder.output([("surface", glow.into())]);
}

fn retuned(
    renderer: &mut Renderer,
    graphed: &Material,
    drawn: MaterialId,
    settings: &[(&str, SocketValue)],
    binding: Option<TextureBinding>,
) {
    let values: Vec<(&str, Value)> = graphed
        .values(
            |at: &SocketRef| {
                settings
                    .iter()
                    .find(|(port, _)| *port == at.port)
                    .map(|(_, value)| value.clone())
            },
            |_| binding,
        )
        .collect();
    renderer
        .three()
        .set_material_values(drawn, &values)
        .expect("the uniforms take the values");
}

fn bright(color: [i32; 3]) -> i32 {
    color.into_iter().max().unwrap_or(0)
}

#[test]
fn toon_step_puts_the_cel_edge_where_the_light_crosses_it() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let facing = builder.external("npr.wrap", [("wrap", 0.0.into())]);
    let facing = builder.out(facing, "value");
    let step = builder.external(
        "npr.toon_step",
        [
            ("value", facing.into()),
            ("edge", 0.5.into()),
            ("feather", 0.0.into()),
        ],
    );
    let step = builder.out(step, "value");
    lighting(&mut builder, step);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let mut renderer = renderer();
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let shot = sphere_of(&mut renderer, &[drawn], Vec3::new(-1.0, 0.0, 0.0));

    assert_eq!(
        rgb(&shot, 37, SIZE / 2),
        [0, 0, 0],
        "N·L below 0.5 is shadow"
    );
    assert_eq!(
        rgb(&shot, 44, SIZE / 2),
        [255, 255, 255],
        "N·L above 0.5 is fully lit"
    );
}

#[test]
fn sdf_face_shadow_follows_the_light_side_and_mirrors() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let uv = builder.input(Input::Uv);
    let across = builder.swizzle(uv, "x");
    let mirrored = builder.op(Op::OneMinus, [across.clone().into()]);
    let face = builder.external(
        "npr.face_sdf",
        [
            ("sdf", across.into()),
            ("mirrored", mirrored.into()),
            ("mirror", 0.0.into()),
            ("softness", 0.0.into()),
        ],
    );
    let face = builder.out(face, "lit");
    lighting(&mut builder, face);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let mut renderer = renderer();
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let screen = quad(&mut renderer, SCREEN_HALF);

    let cases = [
        (Vec3::Z, Vec3::NEG_X, 0.0, [false, true]),
        (Vec3::Z, Vec3::X, 0.0, [true, false]),
        (Vec3::Z, Vec3::NEG_X, 1.0, [true, false]),
        (Vec3::NEG_Z, Vec3::NEG_X, 0.0, [true, false]),
        (Vec3::Z, Vec3::new(-1.0, 0.0, -1.0), 0.0, [true, true]),
    ];
    for (index, (forward, travel, mirror, expected)) in cases.into_iter().enumerate() {
        retuned(
            &mut renderer,
            &graphed,
            drawn,
            &[("mirror", SocketValue::Number(Constant::scalar(mirror)))],
            None,
        );
        let mut frame = frame();
        looked(&mut frame);
        let figure = frame.figure(Figure {
            head: Some(Head {
                center: Vec3::ZERO,
                forward,
                up: Vec3::Y,
                radius: 0.5,
            }),
            ..Figure::default()
        });
        frame.light(
            Light::directional(travel)
                .intensity(2.0)
                .shadow(ShadowQuality::None),
        );
        frame.draw(Draw::new(screen, drawn).figure(figure));
        let shot = pixels(&mut renderer, &frame);
        let lit = [16, 48].map(|x| bright(rgb(&shot, x, SIZE / 2)) > 64);
        assert_eq!(lit, expected, "case {index}: left and right halves");
    }
}

#[test]
fn ramp_atlas_reads_the_chosen_row() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let ramp = builder.external(
        "npr.ramp",
        [
            ("texture", "ramp".into()),
            ("rows", 4.0.into()),
            ("row", 0.0.into()),
            ("origin", 0.0.into()),
            ("second_offset", 0.0.into()),
            ("blend", 0.0.into()),
        ],
    );
    let ramp = builder.out(ramp, "rgb");
    emitting(&mut builder, ramp);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let mut renderer = renderer();
    let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]];
    let binding = texture(&mut renderer, 2, 4, |_, y| {
        let [r, g, b] = colors[y as usize];
        [r as u8, g as u8, b as u8, 255]
    });
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let screen = quad(&mut renderer, SCREEN_HALF);

    let cases = [
        ((2.0, 0.0, 0.0, 0.0), [0, 0, 255]),
        ((0.0, 1.0, 0.0, 0.0), [255, 255, 255]),
        ((1.0, 0.0, 2.0, 1.0), [255, 255, 255]),
        ((1.0, 0.0, 2.0, 0.5), [188, 255, 188]),
    ];
    for ((row, origin, second, blend), expected) in cases {
        let number = |value: f32| SocketValue::Number(Constant::scalar(value));
        retuned(
            &mut renderer,
            &graphed,
            drawn,
            &[
                ("texture", SocketValue::Asset("ramp".to_owned())),
                ("row", number(row)),
                ("origin", number(origin)),
                ("second_offset", number(second)),
                ("blend", number(blend)),
            ],
            Some(binding),
        );
        let mut frame = frame();
        frame.draw(Draw::new(screen, drawn));
        let shot = pixels(&mut renderer, &frame);
        let found = rgb(&shot, SIZE / 2, SIZE / 2);
        assert!(
            near(found, expected, 2),
            "row {row} origin {origin} second {second} blend {blend}: {found:?}"
        );
    }
}

#[test]
fn depth_rim_lights_the_silhouette_on_the_chosen_side() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let rim = builder.external(
        "npr.depth_rim",
        [
            ("width", 60.0.into()),
            ("direction", 1.0.into()),
            ("toward", Arg::from([1.0, 0.0, 0.0])),
        ],
    );
    let rim = builder.out(rim, "mask");
    emitting(&mut builder, rim);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let mut renderer = renderer();
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let mesh = rikka_test_renderer_3d::sphere(&mut renderer, 1.2, 32).expect("a sphere");

    for (direction, expected) in [(1.0, [true, false, true]), (2.0, [false, false, true])] {
        retuned(
            &mut renderer,
            &graphed,
            drawn,
            &[(
                "direction",
                SocketValue::Number(Constant::scalar(direction)),
            )],
            None,
        );
        let mut frame = frame();
        looked(&mut frame);
        frame.draw(Draw::new(mesh, drawn));
        let shot = pixels(&mut renderer, &frame);
        let center = bright(rgb(&shot, SIZE / 2, SIZE / 2));
        let lit = [15, SIZE / 2, 48].map(|x| bright(rgb(&shot, x, SIZE / 2)) > center + 60);
        assert_eq!(
            lit, expected,
            "direction {direction}: left edge, centre, right edge"
        );
    }
}

#[test]
fn kajiya_kay_highlight_runs_across_the_strand() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let strands = builder.external(
        "npr.kajiya_kay",
        [
            ("tangent", Arg::from([1.0, 0.0, 0.0])),
            ("shift", 0.0.into()),
            ("exponent", 64.0.into()),
            ("strength", 1.0.into()),
            ("second_strength", 0.0.into()),
        ],
    );
    let primary = builder.out(strands, "primary");
    lighting(&mut builder, primary);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let mut renderer = renderer();
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));

    for (tangent, vertical) in [([1.0, 0.0, 0.0], true), ([0.0, 1.0, 0.0], false)] {
        retuned(
            &mut renderer,
            &graphed,
            drawn,
            &[(
                "tangent",
                SocketValue::Number(Constant::vec3(tangent[0], tangent[1], tangent[2])),
            )],
            None,
        );
        let shot = sphere_of(&mut renderer, &[drawn], Vec3::NEG_Z);
        let column = [20, 44].map(|y| bright(rgb(&shot, SIZE / 2, y)));
        let row = [20, 44].map(|x| bright(rgb(&shot, x, SIZE / 2)));
        let (along, across) = match vertical {
            true => (column, row),
            false => (row, column),
        };
        assert!(
            along.iter().all(|value| *value > 150) && across.iter().all(|value| *value < 60),
            "tangent {tangent:?}: along {along:?}, across {across:?}"
        );
    }
}

#[test]
fn outline_width_clamps_to_screen_pixels() {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let width = builder.external(
        "npr.outline_width",
        [
            ("width", 0.0001.into()),
            ("min_pixels", 4.0.into()),
            ("max_pixels", 0.0.into()),
        ],
    );
    let width = builder.out(width, "value");
    let body = builder.shader(NodeKind::Emission, [("color", [0.0, 1.0, 0.0].into())]);
    let line = builder.shader(NodeKind::Emission, [("color", [1.0, 0.0, 0.0].into())]);
    builder.output([
        ("surface", body.into()),
        ("outline", line.into()),
        ("outline_width", width.into()),
    ]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let mut renderer = renderer();
    let body = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let hull = material(
        &mut renderer,
        MaterialDesc::new(graphed.template())
            .state(MaterialState::opaque().cull(Cull::Front))
            .hull(),
    );

    for (width, lowest, highest, expected) in [(0.0001, 4.0, 0.0, 4), (1.0, 0.0, 2.0, 2)] {
        let number = |value: f32| SocketValue::Number(Constant::scalar(value));
        let settings = [
            ("width", number(width)),
            ("min_pixels", number(lowest)),
            ("max_pixels", number(highest)),
        ];
        retuned(&mut renderer, &graphed, body, &settings, None);
        retuned(&mut renderer, &graphed, hull, &settings, None);
        let shot = sphere_of(&mut renderer, &[body, hull], Vec3::new(-1.0, -0.3, -1.0));
        let ring = (0..SIZE / 2)
            .filter(|x| rgb(&shot, *x, SIZE / 2) == [255, 0, 0])
            .count() as i32;
        assert!(
            (ring - expected).abs() <= 1,
            "width {width} clamped to {lowest}…{highest} px draws a {ring} px ring"
        );
    }
}
