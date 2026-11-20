use rikka_gpu::{Filtering, Pixels};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::glam::{Vec3, Vec4};
use rikka_renderer_3d::{
    Draw, Figure, Head, Light, MaterialDesc, MaterialId, MeshId, ShadowQuality, TextureBinding,
    Value,
};
use rikka_shader_compile::Material;
use rikka_shader_graph::Library;
use rikka_shader_presets::preset;

use crate::support::{
    SCREEN_HALF, SIZE, compiled, compiler, frame, looked, pixels, quad, renderer, rgb,
};

const RADIUS: f32 = 1.2;
const DISTANCE: f32 = 4.0;
const FROM_RIGHT: Vec3 = Vec3::NEG_X;
const GREY: Vec4 = Vec4::new(0.3, 0.3, 0.3, 1.0);
const DARK_SHADE: Vec3 = Vec3::new(0.2, 0.2, 0.2);
const MARGIN: f32 = 0.25;

fn data(
    renderer: &mut Renderer,
    width: u32,
    height: u32,
    texel: impl Fn(u32, u32) -> [u8; 4],
) -> TextureBinding {
    let bytes: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| texel(x, y))
        .collect();
    let id = renderer
        .textures()
        .create(Pixels::premultiplied(width, height, bytes))
        .expect("a texture");
    TextureBinding::new(id, Filtering::NearestClamp)
}

fn compiled_preset(key: &str) -> Material {
    let library = Library::standard();
    let graph = preset(key)
        .unwrap_or_else(|| panic!("{key} is a preset"))
        .graph(&library)
        .unwrap_or_else(|error| panic!("{key}: {error}"));
    compiled(&graph, &library, &compiler())
}

fn drawn(renderer: &mut Renderer, material: &Material, values: &[(&str, Value)]) -> MaterialId {
    let desc = values
        .iter()
        .try_fold(
            MaterialDesc::new(material.template()),
            |desc, (name, value)| desc.with(name, value.clone()),
        )
        .expect("the preset declares these parameters");
    renderer.three().create_material(desc).expect("a material")
}

fn column(sine: f32) -> u32 {
    let cosine = (1.0 - sine * sine).sqrt();
    let ndc = RADIUS * sine / (DISTANCE - RADIUS * cosine) / 0.5_f32.tan();
    ((ndc * 0.5 + 0.5) * SIZE as f32) as u32
}

fn luma(color: [i32; 3]) -> i32 {
    (color[0] + 2 * color[1] + color[2]) / 4
}

struct Sphere {
    mesh: MeshId,
}

impl Sphere {
    fn new(renderer: &mut Renderer) -> Self {
        Self {
            mesh: rikka_test_renderer_3d::sphere(renderer, RADIUS, 32).expect("a sphere"),
        }
    }

    fn shot(&self, renderer: &mut Renderer, material: MaterialId, travel: Vec3) -> Vec<u8> {
        let mut frame = frame();
        frame.light(
            Light::directional(travel)
                .intensity(2.0)
                .shadow(ShadowQuality::None),
        );
        let figure = frame.figure(Figure {
            head: Some(Head {
                center: Vec3::ZERO,
                forward: Vec3::Z,
                up: Vec3::Y,
                radius: RADIUS,
            }),
            ..Figure::default()
        });
        frame.draw(Draw::new(self.mesh, material).figure(figure));
        pixels(renderer, &frame)
    }
}

fn equator(shot: &[u8], from: f32, to: f32) -> Vec<[i32; 3]> {
    (column(from)..=column(to))
        .map(|x| rgb(shot, x, SIZE / 2))
        .collect()
}

fn spread(tones: &[[i32; 3]]) -> i32 {
    let lumas: Vec<i32> = tones.iter().copied().map(luma).collect();
    lumas.iter().max().unwrap_or(&0) - lumas.iter().min().unwrap_or(&0)
}

fn mean(tones: &[[i32; 3]]) -> [i32; 3] {
    let count = tones.len().max(1) as i32;
    [0, 1, 2].map(|channel| tones.iter().map(|tone| tone[channel]).sum::<i32>() / count)
}

fn assert_cel(key: &str, shot: &[u8], bands: &[f32]) {
    let mut edges = vec![-0.85];
    edges.extend_from_slice(bands);
    edges.push(0.85);
    let plateaus: Vec<[i32; 3]> = edges
        .windows(2)
        .map(|pair| {
            let tones = equator(shot, pair[0] + MARGIN, pair[1] - MARGIN);
            assert!(
                spread(&tones) <= 6,
                "{key}: the band {:.2}…{:.2} holds one tone, found {tones:?}",
                pair[0],
                pair[1]
            );
            mean(&tones)
        })
        .collect();
    for (index, pair) in plateaus.windows(2).enumerate() {
        assert!(
            luma(pair[1]) > luma(pair[0]) + 8,
            "{key}: band {index} {:?} is darker than band {} {:?}",
            pair[0],
            index + 1,
            pair[1]
        );
        let middle = (luma(pair[0]) + luma(pair[1])) / 2;
        let crossing = (column(edges[index + 1] - MARGIN)..=column(edges[index + 1] + MARGIN))
            .find(|x| luma(rgb(shot, *x, SIZE / 2)) >= middle)
            .unwrap_or(0);
        let expected = column(bands[index]) as i32;
        assert!(
            (crossing as i32 - expected).abs() <= 2,
            "{key}: the cel edge {index} lies at x {crossing}, the recipe puts it at {expected}"
        );
    }
}

fn plain() -> Vec<(&'static str, Value)> {
    vec![
        ("base_color", Value::from(GREY)),
        ("shade_color", Value::from(DARK_SHADE)),
    ]
}

fn zenless() -> Vec<(&'static str, Value)> {
    vec![
        ("base_color", Value::from(GREY)),
        ("shade_color", Value::from(Vec3::splat(0.6))),
        ("deep_color", Value::from(Vec3::splat(0.1))),
    ]
}

fn uniform(renderer: &mut Renderer, texel: [u8; 4]) -> TextureBinding {
    data(renderer, 2, 2, |_, _| texel)
}

fn rows(renderer: &mut Renderer, rows: &[[u8; 3]]) -> TextureBinding {
    data(renderer, 4, rows.len() as u32, |_, y| {
        let [r, g, b] = rows[y as usize];
        [r, g, b, 255]
    })
}

fn dominant(color: [i32; 3]) -> usize {
    (0..3).max_by_key(|channel| color[*channel]).unwrap_or(0)
}

#[test]
fn genshin_body_reads_lightmap_ids_ramp_rows_and_forced_light() {
    let mut renderer = renderer();
    let sphere = Sphere::new(&mut renderer);
    let graphed = compiled_preset("genshin_body");
    let bare = drawn(&mut renderer, &graphed, &plain());
    let bare = sphere.shot(&mut renderer, bare, FROM_RIGHT);
    assert_cel("genshin_body", &bare, &[0.2]);

    let ramp = rows(
        &mut renderer,
        &[
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [255, 255, 0],
            [255, 0, 255],
            [255, 255, 255],
            [128, 0, 0],
            [0, 0, 255],
            [128, 128, 0],
            [128, 0, 128],
        ],
    );
    let shadow_side = column(-0.6);
    let lit_side = column(0.6);
    let cases = [
        (128, 1.0, Some(1)),
        (255, 1.0, Some(0)),
        (128, 0.0, Some(2)),
        (128, 0.5, None),
    ];
    for (alpha, day, expected) in cases {
        let map = uniform(&mut renderer, [0, 128, 0, alpha]);
        let mut values = plain();
        values.extend([
            ("lightmap_texture", Value::from(map)),
            ("ramp_texture", Value::from(ramp)),
            ("day", Value::from(day)),
        ]);
        let material = drawn(&mut renderer, &graphed, &values);
        let shot = sphere.shot(&mut renderer, material, FROM_RIGHT);
        let shadow = rgb(&shot, shadow_side, SIZE / 2);
        match expected {
            Some(channel) => assert_eq!(
                dominant(shadow),
                channel,
                "alpha {alpha} day {day}: shadow side {shadow:?}"
            ),
            None => assert!(
                (shadow[1] - shadow[2]).abs() <= 12 && shadow[1] > shadow[0] + 40,
                "alpha {alpha} halfway to night mixes the green day row and the blue night row: {shadow:?}"
            ),
        }
    }

    let [bare_shadow, bare_lit] = [shadow_side, lit_side].map(|x| luma(rgb(&bare, x, SIZE / 2)));
    for (bias, reference, forced) in [(255, bare_lit, "light"), (0, bare_shadow, "shadow")] {
        let map = uniform(&mut renderer, [0, bias, 0, 0]);
        let mut values = plain();
        values.push(("lightmap_texture", Value::from(map)));
        let material = drawn(&mut renderer, &graphed, &values);
        let shot = sphere.shot(&mut renderer, material, FROM_RIGHT);
        let [shadow, lit] = [shadow_side, lit_side].map(|x| luma(rgb(&shot, x, SIZE / 2)));
        assert!(
            (shadow - reference).abs() <= 4 && (lit - reference).abs() <= 4,
            "bias {bias} forces {forced} on both sides: {shadow} and {lit} against {reference}"
        );
    }
}

#[test]
fn star_rail_blends_warm_and_cool_ramps_and_shows_skin_through_stockings() {
    let mut renderer = renderer();
    let sphere = Sphere::new(&mut renderer);
    let hair = compiled_preset("starrail_hair");
    let bare = drawn(&mut renderer, &hair, &plain());
    let bare = sphere.shot(&mut renderer, bare, FROM_RIGHT);
    assert_cel("starrail_hair", &bare, &[-0.253]);

    let graphed = compiled_preset("starrail_body");
    let bare = drawn(&mut renderer, &graphed, &plain());
    let bare = sphere.shot(&mut renderer, bare, FROM_RIGHT);
    assert_cel("starrail_body", &bare, &[-0.253]);

    let warm = rows(&mut renderer, &[[0, 255, 0]; 16]);
    let cool = rows(&mut renderer, &[[255, 0, 0]; 16]);
    for (warmth, expected) in [(1.0, Some(1)), (0.0, Some(0)), (0.5, None)] {
        let mut values = plain();
        values.extend([
            ("ramp_warm_texture", Value::from(warm)),
            ("ramp_cool_texture", Value::from(cool)),
            ("warmth", Value::from(warmth)),
        ]);
        let material = drawn(&mut renderer, &graphed, &values);
        let shot = sphere.shot(&mut renderer, material, FROM_RIGHT);
        let lit = rgb(&shot, column(0.5), SIZE / 2);
        match expected {
            Some(channel) => assert_eq!(dominant(lit), channel, "warmth {warmth}: {lit:?}"),
            None => assert!(
                (lit[0] - lit[1]).abs() <= 12 && lit[0] > lit[2] + 40,
                "warmth {warmth} mixes the cool and warm ramps: {lit:?}"
            ),
        }
    }

    let facing = Vec3::NEG_Z;
    for (stockings, sheer) in [(true, true), (false, false)] {
        let material = drawn(
            &mut renderer,
            &graphed,
            &[
                ("base_color", Value::from(Vec4::new(0.05, 0.05, 0.05, 1.0))),
                ("stockings", Value::from(stockings)),
                ("stocking_skin_color", Value::from(Vec3::splat(0.4))),
                ("stocking_power", Value::from(1.0)),
            ],
        );
        let shot = sphere.shot(&mut renderer, material, facing);
        let [center, edge] = [0.0, 0.85].map(|sine| luma(rgb(&shot, column(sine), SIZE / 2)));
        match sheer {
            true => assert!(
                center > edge + 25,
                "stocking fabric thins towards the view: centre {center}, edge {edge}"
            ),
            false => assert!(
                (center - edge).abs() <= 10,
                "without stockings the lit side stays flat: centre {center}, edge {edge}"
            ),
        }
    }
}

fn redness(color: [i32; 3]) -> i32 {
    color[0] - color[1]
}

#[test]
fn zenless_bands_fringe_and_matcaps_follow_the_recipe() {
    let mut renderer = renderer();
    let sphere = Sphere::new(&mut renderer);
    let hair = compiled_preset("zenless_hair");
    let mut values = zenless();
    values.push(("fringe_color", Value::from(Vec3::ZERO)));
    let bare = drawn(&mut renderer, &hair, &values);
    let bare = sphere.shot(&mut renderer, bare, FROM_RIGHT);
    assert_cel("zenless_hair", &bare, &[-1.0 / 3.0, 1.0 / 3.0]);

    let graphed = compiled_preset("zenless_body");
    let bare = drawn(&mut renderer, &graphed, &values);
    let bare = sphere.shot(&mut renderer, bare, FROM_RIGHT);
    assert_cel("zenless_body", &bare, &[-1.0 / 3.0, 1.0 / 3.0]);

    let mut values = zenless();
    values.push(("fringe_color", Value::from(Vec3::new(0.5, 0.0, 0.0))));
    let fringed = drawn(&mut renderer, &graphed, &values);
    let shot = sphere.shot(&mut renderer, fringed, FROM_RIGHT);
    let edge = (column(0.25)..=column(0.42))
        .map(|x| redness(rgb(&shot, x, SIZE / 2)))
        .max()
        .unwrap_or(0);
    let [shallow, lit] = [0.0, 0.65].map(|sine| redness(rgb(&shot, column(sine), SIZE / 2)));
    assert!(
        edge > shallow.max(lit) + 20,
        "the terminator takes the fringe colour: edge {edge}, shallow {shallow}, lit {lit}"
    );

    let red = uniform(&mut renderer, [255, 0, 0, 255]);
    let green = uniform(&mut renderer, [0, 255, 0, 255]);
    for (id_value, expected) in [(255, Some(0)), (179, Some(1)), (25, None)] {
        let material_map = uniform(&mut renderer, [id_value, 0, 0, 255]);
        let mut values = zenless();
        values.extend([
            ("material_texture", Value::from(material_map)),
            ("matcap_1", Value::from(red)),
            ("matcap_2", Value::from(green)),
        ]);
        let material = drawn(&mut renderer, &graphed, &values);
        let shot = sphere.shot(&mut renderer, material, FROM_RIGHT);
        let lit = rgb(&shot, column(0.6), SIZE / 2);
        match expected {
            Some(channel) => assert!(
                dominant(lit) == channel && lit[channel] > lit[2] + 60,
                "material value {id_value} picks matcap {}: {lit:?}",
                channel + 1
            ),
            None => assert!(
                lit.iter().max().unwrap_or(&0) - lit.iter().min().unwrap_or(&0) <= 4,
                "material value {id_value} has no matcap bound: {lit:?}"
            ),
        }
    }
}

fn face_halves(
    renderer: &mut Renderer,
    material: MaterialId,
    screen: MeshId,
    forward: Vec3,
    travel: Vec3,
) -> ([bool; 2], [i32; 2]) {
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
    frame.draw(Draw::new(screen, material).figure(figure));
    let shot = pixels(renderer, &frame);
    let lumas = [16, 48].map(|x| luma(rgb(&shot, x, SIZE / 2)));
    let [left, right] = lumas;
    let middle = (left + right) / 2;
    let lit = match (left - right).abs() > 20 {
        true => [left > middle, right > middle],
        false => [true, true],
    };
    (lit, lumas)
}

struct FaceMap {
    parameter: &'static str,
    channel: usize,
}

fn assert_face(
    renderer: &mut Renderer,
    sphere: &Sphere,
    key: &str,
    map: FaceMap,
    values: Vec<(&'static str, Value)>,
) {
    let graphed = compiled_preset(key);
    let bare = drawn(renderer, &graphed, &values);
    let shot = sphere.shot(renderer, bare, FROM_RIGHT);
    let [far, near] = [[-0.75, -0.45], [0.45, 0.75]].map(|[from, to]| equator(&shot, from, to));
    assert!(
        spread(&far) <= 6 && luma(mean(&near)) > luma(mean(&far)) + 20,
        "{key} without a face map shades the side away from the light: {far:?} against {near:?}"
    );

    let screen = quad(renderer, SCREEN_HALF);
    let sdf = data(renderer, 64, 1, |x, _| {
        let mut texel = [0, 0, 0, 255];
        texel[map.channel] = (x * 4) as u8;
        texel
    });
    let cases = [
        (Vec3::Z, Vec3::NEG_X, 0, [false, true]),
        (Vec3::Z, Vec3::X, 0, [true, false]),
        (Vec3::Z, Vec3::NEG_X, 1, [true, false]),
        (Vec3::NEG_Z, Vec3::NEG_X, 0, [true, false]),
    ];
    let found: Vec<([bool; 2], [i32; 2])> = cases
        .iter()
        .map(|(forward, travel, mirror, _)| {
            let mut values = values.clone();
            values.extend([
                (map.parameter, Value::from(sdf)),
                (
                    "face_mirror",
                    Value::Enum(["left", "right"][*mirror].to_owned()),
                ),
            ]);
            let material = drawn(renderer, &graphed, &values);
            face_halves(renderer, material, screen, *forward, *travel)
        })
        .collect();
    let expected: Vec<[bool; 2]> = cases.iter().map(|case| case.3).collect();
    let lit: Vec<[bool; 2]> = found.iter().map(|(lit, _)| *lit).collect();
    assert_eq!(
        lit, expected,
        "{key}: lit left and right halves per case, brightness {found:?}"
    );
}

#[test]
fn genshin_face_places_the_sdf_shadow_by_light_side_and_mirror() {
    let mut renderer = renderer();
    let sphere = Sphere::new(&mut renderer);
    let map = FaceMap {
        parameter: "face_map",
        channel: 3,
    };
    assert_face(&mut renderer, &sphere, "genshin_face", map, plain());
}

#[test]
fn star_rail_face_places_the_sdf_shadow_by_light_side_and_mirror() {
    let mut renderer = renderer();
    let sphere = Sphere::new(&mut renderer);
    let map = FaceMap {
        parameter: "face_map",
        channel: 3,
    };
    assert_face(&mut renderer, &sphere, "starrail_face", map, plain());
}

#[test]
fn zenless_face_places_the_sdf_shadow_by_light_side_and_mirror() {
    let mut renderer = renderer();
    let sphere = Sphere::new(&mut renderer);
    let map = FaceMap {
        parameter: "light_texture",
        channel: 0,
    };
    assert_face(&mut renderer, &sphere, "zenless_face", map, zenless());
}
