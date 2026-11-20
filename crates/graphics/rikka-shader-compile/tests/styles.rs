use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::{Vec3, Vec4};
use rikka_renderer_3d::{
    BuiltinTemplate, CompositeInput, Draw, Light, MaterialDesc, MaterialId, MaterialTemplate,
    RenderLayer, ShadowQuality, Value,
};
use rikka_shader_compile::Material;
use rikka_shader_graph::Library;
use rikka_shader_presets::preset;

use crate::support::{SIZE, compiled, compiler, frame, material, pixels, renderer, rgb};

const FROM_RIGHT: Vec3 = Vec3::new(-1.0, 0.0, -0.3);
const FROM_LEFT: Vec3 = Vec3::new(1.0, 0.0, -0.3);

fn graphed(key: &str, library: &Library) -> Material {
    let graph = preset(key)
        .unwrap_or_else(|| panic!("{key} is listed"))
        .graph(library)
        .unwrap_or_else(|error| panic!("{key}: {error}"));
    compiled(&graph, library, &compiler())
}

fn drawn(renderer: &mut Renderer, graphed: &Material, values: &[(&str, Value)]) -> MaterialId {
    let desc = values.iter().fold(
        MaterialDesc::new(graphed.template()),
        |desc, (name, value)| {
            desc.with(name, value.clone())
                .unwrap_or_else(|error| panic!("{name}: {error}"))
        },
    );
    material(renderer, desc)
}

fn sphere(renderer: &mut Renderer, drawn: MaterialId, light: Vec3) -> Vec<u8> {
    let mesh = rikka_test_renderer_3d::sphere(renderer, 1.2, 32).expect("a sphere");
    let mut frame = frame();
    frame.light(
        Light::directional(light)
            .intensity(2.0)
            .shadow(ShadowQuality::None),
    );
    frame.draw(Draw::new(mesh, drawn));
    pixels(renderer, &frame)
}

fn emitted(renderer: &mut Renderer, drawn: MaterialId) -> Vec<u8> {
    let mesh = rikka_test_renderer_3d::sphere(renderer, 1.2, 32).expect("a sphere");
    let mut frame = frame();
    let mut look = rikka_test_renderer_3d::minimal_look();
    look.composite.nodes.clear();
    look.composite.output = CompositeInput::Layer(RenderLayer::Emission);
    frame.look(look);
    frame.light(
        Light::directional(FROM_RIGHT)
            .intensity(2.0)
            .shadow(ShadowQuality::None),
    );
    frame.draw(Draw::new(mesh, drawn));
    pixels(renderer, &frame)
}

fn widest(a: &[u8], b: &[u8]) -> i32 {
    a.chunks(4)
        .zip(b.chunks(4))
        .flat_map(|(a, b)| {
            (0..3).map(move |channel| (i32::from(a[channel]) - i32::from(b[channel])).abs())
        })
        .max()
        .unwrap_or(0)
}

fn row(shot: &[u8]) -> Vec<[i32; 3]> {
    (0..SIZE)
        .map(|x| rgb(shot, x, SIZE / 2))
        .filter(|color| color.iter().any(|channel| *channel > 0))
        .collect()
}

fn plateaus(shot: &[u8]) -> (usize, i32) {
    let greens: Vec<i32> = row(shot).iter().map(|color| color[1]).collect();
    let mut counts: Vec<(i32, usize)> = Vec::new();
    for green in &greens {
        match counts
            .iter_mut()
            .find(|(level, _)| (level - green).abs() <= 2)
        {
            Some((_, count)) => *count += 1,
            None => counts.push((*green, 1)),
        }
    }
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    let covered = counts.iter().take(2).map(|(_, count)| count).sum::<usize>();
    let gap = match counts.as_slice() {
        [first, second, ..] => (first.0 - second.0).abs(),
        _ => 0,
    };
    (greens.len() - covered, gap)
}

fn levels(shot: &[u8]) -> usize {
    let mut greens: Vec<i32> = row(shot).iter().map(|color| color[1]).collect();
    greens.sort_unstable();
    greens.dedup();
    greens.len()
}

fn rgba(r: f32, g: f32, b: f32) -> Value {
    Value::Vec4(Vec4::new(r, g, b, 1.0))
}

fn rgb3(r: f32, g: f32, b: f32) -> Value {
    Value::Vec3(Vec3::new(r, g, b))
}

fn builtin(
    renderer: &mut Renderer,
    template: BuiltinTemplate,
    values: &[(&str, Value)],
) -> MaterialId {
    let template = MaterialTemplate::builtin(template);
    let desc = values
        .iter()
        .fold(MaterialDesc::new(&template), |desc, (name, value)| {
            desc.with(name, value.clone())
                .unwrap_or_else(|error| panic!("{name}: {error}"))
        });
    material(renderer, desc)
}

#[test]
fn hybrid_runs_from_physical_at_zero_to_cel_bands_at_one() {
    let library = Library::standard();
    let mut renderer = renderer();
    let hybrid = graphed("hybrid", &library);

    let physical = builtin(
        &mut renderer,
        BuiltinTemplate::Standard,
        &[
            ("base_color", rgb3(0.8, 0.3, 0.2)),
            ("specular_roughness", Value::Float(0.4)),
        ],
    );
    let physical = sphere(&mut renderer, physical, FROM_RIGHT);
    let zero = drawn(
        &mut renderer,
        &hybrid,
        &[
            ("base_color", rgba(0.8, 0.3, 0.2)),
            ("specular_roughness", Value::Float(0.4)),
            ("stylization", Value::Float(0.0)),
        ],
    );
    let zero = sphere(&mut renderer, zero, FROM_RIGHT);
    let gap = widest(&physical, &zero);
    assert!(
        gap <= 2,
        "hybrid at stylization 0 differs from OpenPBR by {gap}"
    );
    assert!(levels(&zero) > 16, "the physical diffuse is a gradient");

    let one = drawn(
        &mut renderer,
        &hybrid,
        &[
            ("base_color", rgba(0.8, 0.8, 0.8)),
            ("specular_weight", Value::Float(0.0)),
            ("stylization", Value::Float(1.0)),
            ("band_softness", Value::Float(0.0)),
        ],
    );
    let one = sphere(&mut renderer, one, FROM_RIGHT);
    let (stray, step) = plateaus(&one);
    assert!(
        stray <= 4,
        "{stray} pixels of the cel row lie off its two bands"
    );
    assert!(step > 20, "the bands differ by {step}");
}

struct Part<'a> {
    key: &'a str,
    quiet: &'a [(&'a str, Value)],
    template: BuiltinTemplate,
    highlight: &'a [(&'a str, Value)],
    reference: &'a [(&'a str, Value)],
}

fn pairs_physical_highlights_with_two_tone_diffuse(part: Part<'_>) {
    let library = Library::standard();
    let mut renderer = renderer();
    let graph = graphed(part.key, &library);

    let mut cel = vec![
        ("specular_weight", Value::Float(0.0)),
        ("band_softness", Value::Float(0.0)),
        ("terminator_strength", Value::Float(0.0)),
    ];
    cel.extend(part.quiet.iter().cloned());
    let diffuse = drawn(&mut renderer, &graph, &cel);
    let diffuse = sphere(&mut renderer, diffuse, FROM_RIGHT);
    let (stray, step) = plateaus(&diffuse);
    assert!(
        stray <= 4,
        "{}: {stray} pixels lie off the two bands",
        part.key
    );
    assert!(step > 20, "{}: the bands differ by {step}", part.key);

    let mut glossy = vec![("base_color", rgba(0.0, 0.0, 0.0))];
    glossy.extend(part.quiet.iter().cloned());
    glossy.extend(part.highlight.iter().cloned());
    let ours = drawn(&mut renderer, &graph, &glossy);
    let ours = sphere(&mut renderer, ours, FROM_RIGHT);
    let mut reference = vec![("base_color", rgb3(0.0, 0.0, 0.0))];
    reference.extend(part.reference.iter().cloned());
    let theirs = builtin(&mut renderer, part.template, &reference);
    let theirs = sphere(&mut renderer, theirs, FROM_RIGHT);
    assert!(
        row(&theirs).len() > 4,
        "{}: the highlight is visible",
        part.key
    );
    let gap = widest(&ours, &theirs);
    assert!(
        gap <= 2,
        "{}: the highlight differs from {} by {gap}",
        part.key,
        part.template.id()
    );
}

#[test]
fn endfield_body_pairs_openpbr_highlights_with_two_tone_diffuse() {
    pairs_physical_highlights_with_two_tone_diffuse(Part {
        key: "endfield_body",
        quiet: &[],
        template: BuiltinTemplate::Standard,
        highlight: &[],
        reference: &[("specular_roughness", Value::Float(0.5))],
    });
}

#[test]
fn endfield_skin_pairs_dual_lobe_highlights_with_two_tone_diffuse() {
    pairs_physical_highlights_with_two_tone_diffuse(Part {
        key: "endfield_skin",
        quiet: &[
            ("subsurface_mix", Value::Float(0.0)),
            ("rim_shadow_strength", Value::Float(0.0)),
        ],
        template: BuiltinTemplate::Skin,
        highlight: &[],
        reference: &[
            ("specular_weight", Value::Float(0.45)),
            ("specular_roughness", Value::Float(0.55)),
        ],
    });
}

#[test]
fn endfield_hair_pairs_marschner_highlights_with_two_tone_diffuse() {
    pairs_physical_highlights_with_two_tone_diffuse(Part {
        key: "endfield_hair",
        quiet: &[("band_strength", Value::Float(0.0))],
        template: BuiltinTemplate::Hair,
        highlight: &[("specular_weight", Value::Float(1.0))],
        reference: &[("backlit", Value::Float(0.0))],
    });
}

#[test]
fn endfield_face_pairs_openpbr_highlights_with_two_tone_diffuse() {
    pairs_physical_highlights_with_two_tone_diffuse(Part {
        key: "endfield_face",
        quiet: &[],
        template: BuiltinTemplate::Standard,
        highlight: &[],
        reference: &[
            ("specular_weight", Value::Float(0.3)),
            ("specular_roughness", Value::Float(0.55)),
            ("specular_ior", Value::Float(1.4)),
        ],
    });
}

#[test]
fn halftone_inks_only_shadow_and_hatching_thickens_with_darkness() {
    let library = Library::standard();
    let mut renderer = renderer();
    let halftone = graphed("halftone", &library);
    let printed = drawn(&mut renderer, &halftone, &[]);
    let printed = sphere(&mut renderer, printed, FROM_RIGHT);
    let paper = rgb(&printed, 42, SIZE / 2);
    let lit_side: Vec<[i32; 3]> = (38..46)
        .flat_map(|x| (26..38).map(move |y| (x, y)))
        .map(|(x, y)| rgb(&printed, x, y))
        .collect();
    assert!(
        lit_side.iter().all(|color| color == &paper),
        "the lit side is flat colour"
    );
    let shadow: Vec<i32> = (16..24)
        .flat_map(|x| (26..38).map(move |y| (x, y)))
        .map(|(x, y)| rgb(&printed, x, y)[1])
        .collect();
    let inked = shadow.iter().filter(|green| **green < paper[1] / 3).count();
    assert!(
        inked > shadow.len() / 8 && inked < shadow.len() * 7 / 8,
        "the shadow is a dot screen: {inked} of {} inked",
        shadow.len()
    );

    let hatching = graphed("hatching", &library);
    let drawn_lines = drawn(
        &mut renderer,
        &hatching,
        &[("hatch_space", Value::Enum("screen".to_owned()))],
    );
    let drawn_lines = sphere(&mut renderer, drawn_lines, FROM_RIGHT);
    let ink = |columns: std::ops::Range<u32>| {
        let all: Vec<i32> = columns
            .flat_map(|x| (SIZE * 3 / 8..SIZE * 5 / 8).map(move |y| (x, y)))
            .map(|(x, y)| rgb(&drawn_lines, x, y)[1])
            .collect();
        all.iter().filter(|green| **green < 128).count() as f32 / all.len() as f32
    };
    let [bright, middle, dark] = [
        ink(SIZE * 5 / 8..SIZE * 3 / 4),
        ink(SIZE * 7 / 16..SIZE * 9 / 16),
        ink(SIZE / 4..SIZE * 3 / 8),
    ];
    assert!(
        bright < middle && middle < dark,
        "ink rises with darkness: {bright} {middle} {dark}"
    );
}

#[test]
fn gooch_flat_and_glow_keep_their_traits() {
    let library = Library::standard();
    let mut renderer = renderer();
    let gooch = graphed("gooch", &library);
    let shaded = drawn(
        &mut renderer,
        &gooch,
        &[("highlight_strength", Value::Float(0.0))],
    );
    let shaded = sphere(&mut renderer, shaded, FROM_RIGHT);
    let [warm_red, _, warm_blue] = rgb(&shaded, 44, SIZE / 2);
    let [cool_red, _, cool_blue] = rgb(&shaded, 20, SIZE / 2);
    assert!(warm_red > warm_blue, "the lit side is warm");
    assert!(cool_blue > cool_red, "the far side is cool");

    let flat = graphed("flat", &library);
    let plain = drawn(&mut renderer, &flat, &[("base_color", rgba(0.2, 0.6, 0.9))]);
    let right = sphere(&mut renderer, plain, FROM_RIGHT);
    let left = sphere(&mut renderer, plain, FROM_LEFT);
    assert!(row(&right).len() > 16);
    assert_eq!(right, left, "flat colour ignores the light");

    let glow = graphed("glow", &library);
    let energy = drawn(&mut renderer, &glow, &[]);
    let layer = emitted(&mut renderer, energy);
    let center = rgb(&layer, SIZE / 2, SIZE / 2);
    assert!(
        center[2] > 32,
        "the glow reaches the emission layer: {center:?}"
    );
    let matte = drawn(&mut renderer, &gooch, &[]);
    let layer = emitted(&mut renderer, matte);
    assert_eq!(
        rgb(&layer, SIZE / 2, SIZE / 2),
        [0, 0, 0],
        "lit shading leaves the emission layer dark"
    );
}
