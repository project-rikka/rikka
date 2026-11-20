use rikka_renderer::Renderer;
use rikka_renderer_3d::glam::{Vec3, Vec4};
use rikka_renderer_3d::{
    BuiltinTemplate, Draw, Light, MaterialDesc, MaterialId, MaterialTemplate, ShadowQuality, Value,
};
use rikka_shader_compile::Material;
use rikka_shader_graph::Library;
use rikka_shader_presets::preset;

use crate::support::{
    SCREEN_HALF, compiled, compiler, frame, looked, material, pixels, quad, renderer, sphere_of,
};

const LIGHT: Vec3 = Vec3::new(-1.0, -0.3, -1.0);

fn graphed(key: &str, library: &Library) -> Material {
    let graph = preset(key)
        .unwrap_or_else(|| panic!("{key} is listed"))
        .graph(library)
        .unwrap_or_else(|error| panic!("{key}: {error}"));
    compiled(&graph, library, &compiler())
}

fn described(template: &MaterialTemplate, values: &[(&str, Value)]) -> MaterialDesc {
    values
        .iter()
        .fold(MaterialDesc::new(template), |desc, (name, value)| {
            desc.with(name, value.clone())
                .unwrap_or_else(|error| panic!("{name}: {error}"))
        })
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

fn lit(shot: &[u8]) -> usize {
    shot.chunks(4)
        .filter(|texel| texel[..3].iter().any(|channel| *channel > 8))
        .count()
}

#[derive(Debug)]
enum Stage {
    Sphere,
    Looked,
    Card,
}

fn shot(renderer: &mut Renderer, stage: &Stage, drawn: MaterialId) -> Vec<u8> {
    let (mesh, light) = match stage {
        Stage::Sphere => return sphere_of(renderer, &[drawn], LIGHT),
        Stage::Looked => (
            rikka_test_renderer_3d::sphere(renderer, 1.2, 24).expect("a sphere"),
            LIGHT,
        ),
        Stage::Card => (quad(renderer, SCREEN_HALF), Vec3::new(-0.4, -0.5, -1.0)),
    };
    let mut frame = frame();
    if matches!(stage, Stage::Looked) {
        looked(&mut frame);
    }
    frame.light(
        Light::directional(light)
            .intensity(2.0)
            .shadow(ShadowQuality::None),
    );
    frame.draw(Draw::new(mesh, drawn));
    pixels(renderer, &frame)
}

type Values<'a> = &'a [(&'a str, Value)];

struct Pairing<'a> {
    key: &'a str,
    builtin: BuiltinTemplate,
    cases: &'a [(&'a [Stage], Values<'a>, Values<'a>)],
}

fn matches_template(pairings: &[Pairing<'_>]) {
    let library = Library::standard();
    let mut renderer = renderer();
    let mut differing = Vec::new();
    for pairing in pairings {
        let graph = graphed(pairing.key, &library);
        let template = MaterialTemplate::builtin(pairing.builtin);
        for (index, (stages, from_graph, from_template)) in pairing.cases.iter().enumerate() {
            let ours = material(&mut renderer, described(graph.template(), from_graph));
            let theirs = material(&mut renderer, described(&template, from_template));
            for stage in *stages {
                let ours = shot(&mut renderer, stage, ours);
                let theirs = shot(&mut renderer, stage, theirs);
                assert!(
                    lit(&theirs) > 64,
                    "{} case {index} is on screen",
                    pairing.key
                );
                let gap = widest(&ours, &theirs);
                if gap > 2 {
                    differing.push(format!(
                        "{} case {index} {stage:?} differs from {} by {gap} levels",
                        pairing.key,
                        pairing.builtin.id()
                    ));
                }
            }
        }
    }
    assert!(differing.is_empty(), "{}", differing.join("\n"));
}

fn rgb(r: f32, g: f32, b: f32) -> Value {
    Value::Vec3(Vec3::new(r, g, b))
}

fn rgba(r: f32, g: f32, b: f32) -> Value {
    Value::Vec4(Vec4::new(r, g, b, 1.0))
}

#[test]
fn pbr_preset_draws_like_the_standard_template() {
    matches_template(&[Pairing {
        key: "pbr",
        builtin: BuiltinTemplate::Standard,
        cases: &[
            (
                &[Stage::Sphere, Stage::Looked],
                &[
                    ("base_color", rgba(0.8, 0.3, 0.2)),
                    ("specular_roughness", Value::Float(0.4)),
                ],
                &[
                    ("base_color", rgb(0.8, 0.3, 0.2)),
                    ("specular_roughness", Value::Float(0.4)),
                ],
            ),
            (
                &[Stage::Sphere],
                &[
                    ("base_color", rgba(0.9, 0.6, 0.3)),
                    ("base_metalness", Value::Float(1.0)),
                    ("specular_roughness", Value::Float(0.25)),
                    ("specular_roughness_anisotropy", Value::Float(0.6)),
                    ("specular_rotation", Value::Float(0.7)),
                    ("coat_weight", Value::Float(0.7)),
                    ("coat_roughness", Value::Float(0.1)),
                ],
                &[
                    ("base_color", rgb(0.9, 0.6, 0.3)),
                    ("base_metalness", Value::Float(1.0)),
                    ("specular_roughness", Value::Float(0.25)),
                    ("specular_roughness_anisotropy", Value::Float(0.6)),
                    ("specular_rotation", Value::Float(0.7)),
                    ("coat_weight", Value::Float(0.7)),
                    ("coat_roughness", Value::Float(0.1)),
                ],
            ),
            (
                &[Stage::Sphere],
                &[
                    ("base_color", rgba(0.3, 0.5, 0.8)),
                    ("fuzz_weight", Value::Float(0.8)),
                    ("fuzz_color", rgb(1.0, 0.7, 0.8)),
                    ("thin_film_weight", Value::Float(0.6)),
                    ("subsurface_weight", Value::Float(0.5)),
                    ("emission_color", rgb(0.1, 0.05, 0.0)),
                ],
                &[
                    ("base_color", rgb(0.3, 0.5, 0.8)),
                    ("fuzz_weight", Value::Float(0.8)),
                    ("fuzz_color", rgb(1.0, 0.7, 0.8)),
                    ("thin_film_weight", Value::Float(0.6)),
                    ("subsurface_weight", Value::Float(0.5)),
                    ("emission_color", rgb(0.1, 0.05, 0.0)),
                    ("emission_luminance", Value::Float(32_000.0)),
                ],
            ),
            (
                &[Stage::Looked],
                &[
                    ("base_color", rgba(0.3, 0.5, 0.8)),
                    ("subsurface_weight", Value::Float(0.5)),
                ],
                &[
                    ("base_color", rgb(0.3, 0.5, 0.8)),
                    ("subsurface_weight", Value::Float(0.5)),
                ],
            ),
        ],
    }]);
}

#[test]
fn cloth_and_glass_presets_draw_like_the_standard_template() {
    matches_template(&[
        Pairing {
            key: "pbr_cloth",
            builtin: BuiltinTemplate::Standard,
            cases: &[
                (
                    &[Stage::Sphere],
                    &[],
                    &[
                        ("base_diffuse_roughness", Value::Float(0.5)),
                        ("specular_weight", Value::Float(0.5)),
                        ("specular_roughness", Value::Float(0.7)),
                        ("fuzz_weight", Value::Float(0.6)),
                    ],
                ),
                (
                    &[Stage::Sphere],
                    &[
                        ("base_color", rgba(0.55, 0.08, 0.12)),
                        ("specular_weight", Value::Float(1.0)),
                        ("specular_roughness", Value::Float(0.35)),
                        ("specular_roughness_anisotropy", Value::Float(0.8)),
                        ("fuzz_weight", Value::Float(0.25)),
                    ],
                    &[
                        ("base_color", rgb(0.55, 0.08, 0.12)),
                        ("base_diffuse_roughness", Value::Float(0.5)),
                        ("specular_roughness", Value::Float(0.35)),
                        ("specular_roughness_anisotropy", Value::Float(0.8)),
                        ("fuzz_weight", Value::Float(0.25)),
                    ],
                ),
            ],
        },
        Pairing {
            key: "pbr_glass",
            builtin: BuiltinTemplate::Standard,
            cases: &[(
                &[Stage::Sphere],
                &[],
                &[
                    ("base_color", rgb(1.0, 1.0, 1.0)),
                    ("specular_roughness", Value::Float(0.0)),
                    ("transmission_weight", Value::Float(1.0)),
                    ("transmission_dispersion_scale", Value::Float(0.5)),
                    ("geometry_thickness", Value::Float(0.05)),
                ],
            )],
        },
    ]);
}

#[test]
fn skin_hair_and_eye_presets_draw_like_their_templates() {
    matches_template(&[
        Pairing {
            key: "pbr_skin",
            builtin: BuiltinTemplate::Skin,
            cases: &[
                (&[Stage::Sphere, Stage::Looked], &[], &[]),
                (
                    &[Stage::Sphere],
                    &[
                        ("specular_roughness", Value::Float(0.4)),
                        ("fuzz_weight", Value::Float(0.3)),
                        ("thickness", Value::Float(0.01)),
                    ],
                    &[
                        ("specular_roughness", Value::Float(0.4)),
                        ("fuzz_weight", Value::Float(0.3)),
                        ("thickness", Value::Float(0.01)),
                    ],
                ),
            ],
        },
        Pairing {
            key: "pbr_hair",
            builtin: BuiltinTemplate::Hair,
            cases: &[
                (&[Stage::Card], &[], &[]),
                (
                    &[Stage::Card],
                    &[
                        ("direction", Value::Enum("u".to_owned())),
                        ("tip_color", rgb(1.0, 0.8, 0.6)),
                        ("roughness", Value::Float(0.25)),
                    ],
                    &[
                        ("direction", Value::Enum("u".to_owned())),
                        ("tip_color", rgb(1.0, 0.8, 0.6)),
                        ("roughness", Value::Float(0.25)),
                    ],
                ),
            ],
        },
        Pairing {
            key: "pbr_eye",
            builtin: BuiltinTemplate::Eye,
            cases: &[(
                &[Stage::Card],
                &[("iris_radius", [0.3, 0.3].into())],
                &[("iris_radius", [0.3, 0.3].into())],
            )],
        },
    ]);
}
