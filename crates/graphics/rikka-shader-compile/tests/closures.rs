use std::time::{Duration, Instant};

use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{MaterialDesc, Three};
use rikka_shader_graph::{
    Arg, Builder, DirectionSpace, Graph, GraphKind, Input, Library, NodeKind, Op,
};

use crate::support::{
    SIZE, built, compiled, compiler, encoded, lit, material, near, renderer, rgb, sphere, sphere_of,
};

const LIGHT: Vec3 = Vec3::new(-1.0, -0.3, -1.0);

const BUILD_LIMIT: Duration = Duration::from_secs(60);

const RESIDENT_LIMIT: u64 = 6 << 30;

fn closure<const N: usize>(
    renderer: &mut Renderer,
    id: &str,
    inputs: [(&str, Arg); N],
    light: Vec3,
) -> Vec<u8> {
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let node = builder.external(id, inputs);
    let shader = builder.out(node, "shader");
    builder.output([("surface", shader.into())]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let drawn = material(renderer, MaterialDesc::new(graphed.template()));
    sphere_of(renderer, &[drawn], light)
}

fn alike(a: &[u8], b: &[u8], tolerance: i32) -> Option<(u32, u32)> {
    (0..SIZE * SIZE)
        .map(|at| (at % SIZE, at / SIZE))
        .find(|(x, y)| !near(rgb(a, *x, *y), rgb(b, *x, *y), tolerance))
}

fn matches_principled<const N: usize, const M: usize>(
    id: &str,
    inputs: [(&str, Arg); N],
    reference: [(&str, Arg); M],
) {
    let mut renderer = renderer();
    let found = closure(&mut renderer, id, inputs, LIGHT);
    let reference = closure(&mut renderer, "bsdf.principled", reference, LIGHT);
    assert!(
        rgb(&found, SIZE / 2, SIZE / 2) != [0, 0, 0],
        "{id} draws something"
    );
    if let Some((x, y)) = alike(&found, &reference, 3) {
        panic!(
            "{id} differs from its reference at {x},{y}: {:?} against {:?}",
            rgb(&found, x, y),
            rgb(&reference, x, y)
        );
    }
}

#[test]
fn diffuse_matches_a_principled_without_specular() {
    matches_principled(
        "bsdf.diffuse",
        [("color", [0.7, 0.4, 0.2].into()), ("roughness", 0.5.into())],
        [
            ("base_color", [0.7, 0.4, 0.2].into()),
            ("diffuse_roughness", 0.5.into()),
            ("specular_weight", 0.0.into()),
        ],
    );
}

#[test]
fn glossy_matches_a_white_principled_metal() {
    matches_principled(
        "bsdf.glossy",
        [("color", [1.0, 1.0, 1.0].into()), ("roughness", 0.4.into())],
        [
            ("base_color", [1.0, 1.0, 1.0].into()),
            ("metalness", 1.0.into()),
            ("roughness", 0.4.into()),
        ],
    );
}

#[test]
fn sheen_matches_principled_fuzz_over_black() {
    matches_principled(
        "bsdf.sheen",
        [("color", [0.9, 0.6, 0.8].into()), ("roughness", 0.4.into())],
        [
            ("base_color", [0.0, 0.0, 0.0].into()),
            ("specular_weight", 0.0.into()),
            ("fuzz_weight", 1.0.into()),
            ("fuzz_color", [0.9, 0.6, 0.8].into()),
            ("fuzz_roughness", 0.4.into()),
        ],
    );
}

#[test]
fn translucent_lights_the_back_like_a_flipped_diffuse() {
    let mut renderer = renderer();
    let behind = Vec3::new(0.3, 0.2, 1.0);
    let through = closure(
        &mut renderer,
        "bsdf.translucent",
        [("color", [0.6, 0.8, 0.4].into())],
        behind,
    );
    let library = Library::standard();
    let mut builder = Builder::new(GraphKind::Material, &library);
    let normal = builder.input(Input::Normal(DirectionSpace::World));
    let flipped = builder.op(Op::Negate, [normal.into()]);
    let node = builder.external(
        "bsdf.diffuse",
        [
            ("color", [0.6, 0.8, 0.4].into()),
            ("normal", flipped.into()),
        ],
    );
    let shader = builder.out(node, "shader");
    builder.output([("surface", shader.into())]);
    let graph = builder.finish().expect("a valid graph");
    let graphed = compiled(&graph, &library, &compiler());
    let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
    let reference = sphere_of(&mut renderer, &[drawn], behind);

    let center = rgb(&through, SIZE / 2, SIZE / 2);
    assert!(
        center[1] > 100,
        "light from behind passes through: {center:?}"
    );
    assert_eq!(alike(&through, &reference, 2), None);
}

#[test]
fn toon_closure_is_flat_within_its_size_and_normalized_like_cycles() {
    let mut renderer = renderer();
    let size: f32 = 0.2;
    let shot = closure(
        &mut renderer,
        "bsdf.toon",
        [
            ("color", [0.5, 0.5, 0.5].into()),
            ("size", size.into()),
            ("smooth", 0.0.into()),
        ],
        Vec3::NEG_Z,
    );
    let edge = size * std::f32::consts::FRAC_PI_2;
    let normalized = 1.0 / (std::f32::consts::TAU * (1.0 - edge.cos()));
    let ambient = 0.2;
    let lit = encoded(0.5 * (2.0 * normalized + ambient));
    let unlit = encoded(0.5 * ambient);
    assert!(
        near(rgb(&shot, SIZE / 2, SIZE / 2), [lit; 3], 2),
        "the centre faces the light: {:?} against {lit}",
        rgb(&shot, SIZE / 2, SIZE / 2)
    );
    assert!(
        near(rgb(&shot, SIZE / 2 + 12, SIZE / 2), [unlit; 3], 2),
        "35° off the light is outside a 18° toon size: {:?} against {unlit}",
        rgb(&shot, SIZE / 2 + 12, SIZE / 2)
    );
}

fn layered(library: &Library, id: &str, count: usize) -> Graph {
    let mut builder = Builder::new(GraphKind::Material, library);
    let mut shaders: Vec<Arg> = (0..count)
        .map(|_| {
            let (color, layer) = match id {
                "bsdf.skin" => ("subsurface_color", "fuzz_weight"),
                _ => ("base_color", "coat_weight"),
            };
            let node = builder.external(
                id,
                [
                    (color, [0.8, 0.45, 0.3].into()),
                    ("roughness", 0.4.into()),
                    (layer, 0.3.into()),
                ],
            );
            builder.out(node, "shader").into()
        })
        .collect();
    while shaders.len() > 1 {
        let pairs: Vec<Arg> = shaders
            .chunks(2)
            .map(|pair| match pair {
                [a, b] => builder
                    .shader(
                        NodeKind::MixShader,
                        [("a", a.clone()), ("b", b.clone()), ("factor", 0.5.into())],
                    )
                    .into(),
                [single] => single.clone(),
                _ => unreachable!("chunks of two"),
            })
            .collect();
        shaders = pairs;
    }
    builder.output([("surface", shaders.remove(0))]);
    builder.finish().expect("a valid graph")
}

fn peak_resident() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("procfs");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| {
            rest.trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<u64>()
                .ok()
        })
        .expect("VmHWM in kB")
        * 1024
}

fn layers_draw_like_one(id: &str, looked: bool) {
    let mut renderer = renderer();
    let library = Library::standard();
    let compiler = compiler();
    let mesh = sphere(&mut renderer);
    let mut shots = Vec::new();
    for count in [1, 2, 4] {
        let start = Instant::now();
        let graphed = compiled(&layered(&library, id, count), &library, &compiler);
        let drawn = material(&mut renderer, MaterialDesc::new(graphed.template()));
        let mut frame = lit(mesh, &[drawn], LIGHT);
        if looked {
            frame.look(rikka_test_renderer_3d::minimal_look());
        }
        shots.push(built(&mut renderer, &frame, graphed.template()));
        let took = start.elapsed();
        println!(
            "{count} × {id}: built in {took:?}, peak resident {} MiB",
            peak_resident() >> 20
        );
        assert!(took < BUILD_LIMIT, "{count} × {id} took {took:?} to build");
        renderer
            .three()
            .destroy_material(drawn)
            .expect("a live material");
    }
    assert!(
        peak_resident() < RESIDENT_LIMIT,
        "building layered closures peaked at {} MiB",
        peak_resident() >> 20
    );
    assert!(rgb(&shots[0], SIZE / 2, SIZE / 2) != [0, 0, 0]);
    for (shot, count) in shots[1..].iter().zip([2, 4]) {
        if let Some((x, y)) = alike(shot, &shots[0], 1) {
            panic!(
                "{count} even layers of {id} differ from one at {x},{y}: {:?} against {:?}",
                rgb(shot, x, y),
                rgb(&shots[0], x, y)
            );
        }
    }
}

#[test]
fn even_mixes_of_principled_closures_build_quickly_and_draw_like_one() {
    layers_draw_like_one("bsdf.principled", false);
}

#[test]
fn even_mixes_of_scattered_skin_closures_draw_like_one() {
    layers_draw_like_one("bsdf.skin", true);
}
