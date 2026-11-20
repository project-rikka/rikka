use rikka_shader_graph::{
    Arg, Builder, Constant, Error, Graph, GraphKind, Input, Library, NodeKind, Op, ValueType, Wire,
};
use rikka_slang::TextureKind;

use crate::games::{
    Compare, Defaults, FACE, FACE_SHADOW, OUTLINE, OutlineStyle, RIM, SHADE, SPECULAR, alpha_use,
    ambient_strength, base, call, choice, color, combine, common, compare, face_shadow_group,
    flat_ambient, float, group, imports, incoming, map, maximum, node, one_minus, optional_map,
    outline_shape, power, unit, vertex_controls,
};
use crate::{Family, Preset};

const MAPS: &str = "ZZZ Maps";
const SHADOW_VALUE: &str = "ZZZ Shadow Value";
const BANDS: &str = "ZZZ Bands";
const MATCAPS: &str = "ZZZ Matcaps";
const SPECULAR_GROUP: &str = "ZZZ Specular";
const HAIR_BAND: &str = "ZZZ Hair Band";
const DEPTH_RIM: &str = "ZZZ Rim";

const MAPS_GROUP: &str = "Maps";
const MATCAP_GROUP: &str = "Matcaps";
const HAIR_GROUP: &str = "Hair highlight";
const MATCAP_SLOTS: usize = 5;
const MATCAP_MODES: &[&str] = &["add", "multiply", "mix", "overlay", "screen"];
const THIRD: f32 = 1.0 / 3.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Body,
    Hair,
    Face,
}

pub(super) fn presets() -> Vec<Preset> {
    vec![
        Preset {
            key: "zenless_body",
            title: "Zenless Zone Zero body",
            summary: "Analytic three-band shadow (lit, shallow, deep) with a coloured terminator fringe and saturation-preserving shade colours, up to five matcaps chosen by material ID, a near-step toon GGX highlight, an upward depth rim and coloured lit outlines.",
            family: Family::Game,
            build: body,
        },
        Preset {
            key: "zenless_hair",
            title: "Zenless Zone Zero hair",
            summary: "The Zenless band shading with the painted hair highlight revealed by lighting on a sphere around the head, so it sweeps over the hair as one ring.",
            family: Family::Game,
            build: hair,
        },
        Preset {
            key: "zenless_face",
            title: "Zenless Zone Zero face",
            summary: "SDF face shadow re-banded by the three-band shade, cheek and nose highlight, nose line near the frontal view, chin occlusion and coloured lit outline.",
            family: Family::Game,
            build: face,
        },
    ]
}

fn body(library: &Library) -> Result<Graph, Error> {
    build(library, Part::Body)
}

fn hair(library: &Library) -> Result<Graph, Error> {
    build(library, Part::Hair)
}

fn face(library: &Library) -> Result<Graph, Error> {
    build(library, Part::Face)
}

const DEFAULTS: Defaults = Defaults {
    shade: [0.84, 0.74, 0.84],
    outline: [0.42, 0.26, 0.26],
    outline_width: 0.001,
};

fn groups(b: &mut Builder<'_>) {
    common(b);
    face_shadow_group(b);
    let scalar = |value: f32| Some(Constant::scalar(value));
    let white = Some(Constant::vec3(1.0, 1.0, 1.0));
    group(
        b,
        MAPS,
        "Splits ZZZ's body maps, each optional: light (_N: RG tangent normal with z rebuilt, B shadow bias 0.5 neutral, A outline width), material (_M: R material ID 0…4 counted down from 1, G metallic, B specular mask or the hair's painted band), mask (_A: G smoothness, B emission and matcap mask). Missing maps read (0.5, 0.5, 0.5, 1), (1, 0, 0, 1) and (1, 0.5, 0, 1): mesh normal, no bias, ID 0, no metal or specular, no emission; matcaps then cover their ID.",
        &[
            ("light", ValueType::Texture, None),
            ("material", ValueType::Texture, None),
            ("mask", ValueType::Texture, None),
            ("uv", ValueType::Float2, None),
            ("normal", ValueType::Float3, None),
            ("strength", ValueType::Float, scalar(1.0)),
        ],
        &[
            ("normal", ValueType::Float3),
            ("bias", ValueType::Float),
            ("outline", ValueType::Float),
            ("id", ValueType::Float),
            ("metallic", ValueType::Float),
            ("spec", ValueType::Float),
            ("smoothness", ValueType::Float),
            ("glow", ValueType::Float),
            ("matcap_mask", ValueType::Float),
        ],
        |b| {
            let light = b.function_input("light");
            let material = b.function_input("material");
            let mask = b.function_input("mask");
            let uv = b.function_input("uv");
            let mesh_normal = b.function_input("normal");
            let strength = b.function_input("strength");
            let (light, light_bound) = optional_map(b, &light, [0.5, 0.5, 0.5, 1.0], &uv);
            let (material, _) = optional_map(b, &material, [1.0, 0.0, 0.0, 1.0], &uv);
            let (mask, mask_bound) = optional_map(b, &mask, [1.0, 0.5, 0.0, 1.0], &uv);
            let planar = b.swizzle(light.clone(), "rg");
            let planar = b.multiply(planar, 2.0);
            let planar = b.subtract(planar, 1.0);
            let length = b.dot(planar.clone(), planar.clone());
            let height = one_minus(b, length);
            let height = maximum(b, height, 0.0);
            let height = b.op(Op::SquareRoot, [height.into()]);
            let x = b.swizzle(planar.clone(), "x");
            let y = b.swizzle(planar, "y");
            let tangent = combine(b, x, y, height);
            let mapped = b.op(Op::NormalMap, [tangent.into(), strength.into()]);
            let normal = b.lerp(mesh_normal, mapped, light_bound);
            let normal = b.op(Op::Normalize, [normal.into()]);
            let bias = b.swizzle(light.clone(), "b");
            let outline = b.swizzle(light, "a");
            let id_value = b.swizzle(material.clone(), "r");
            let id = node(
                b,
                "npr.material_id",
                [
                    ("value", id_value.into()),
                    ("mode", 2.0.into()),
                    ("count", 5.0.into()),
                ],
            );
            let id = b.out(id, "index");
            let metallic = b.swizzle(material.clone(), "g");
            let spec = b.swizzle(material, "b");
            let smoothness = b.swizzle(mask.clone(), "g");
            let painted = b.swizzle(mask, "b");
            let glow = b.subtract(painted.clone(), 0.2);
            let glow = b.multiply(glow, 1.25);
            let glow = b.saturate(glow);
            let matcap_mask = b.lerp(1.0, painted, mask_bound);
            b.function_output("normal", normal);
            b.function_output("bias", bias);
            b.function_output("outline", outline);
            b.function_output("id", id);
            b.function_output("metallic", metallic);
            b.function_output("spec", spec);
            b.function_output("smoothness", smoothness);
            b.function_output("glow", glow);
            b.function_output("matcap_mask", matcap_mask);
        },
    );
    group(
        b,
        SHADOW_VALUE,
        "ZZZ's shadow value for the light being added: x = 2 × (2 × bias × shadow - 1) + N·L × shadow, so with the neutral bias 0.5 and no cast shadow x = N·L; the band edges sit at ±1/3 and cast shadow drives x to the deep band.",
        &[
            ("normal", ValueType::Float3, None),
            ("bias", ValueType::Float, scalar(0.5)),
        ],
        &[("value", ValueType::Float)],
        |b| {
            let normal = b.function_input("normal");
            let bias = b.function_input("bias");
            let facing = node(
                b,
                "npr.wrap",
                [("normal", normal.into()), ("wrap", 0.0.into())],
            );
            let facing = b.out(facing, "value");
            let shadow = b.input(Input::Shadow);
            let facing = b.multiply(facing, shadow.clone());
            let value = node(
                b,
                "npr.threshold",
                [
                    ("facing", facing.into()),
                    ("bias", bias.into()),
                    ("mode", 2.0.into()),
                    ("occlusion", shadow.into()),
                    ("shift", (-2.0).into()),
                    ("scale", 4.0.into()),
                ],
            );
            let value = b.out(value, "value");
            b.function_output("value", value);
        },
    );
    group(
        b,
        BANDS,
        "ZZZ's three-band shade multiplier from a shadow value: 1 above +1/3, shallow between ±1/3, deep below, each edge feathered over smoothness; fringe × the lit edge's weight (feathered over fringe_width) adds the coloured terminator; the sum is pushed to its hue keeping brightness (0.4372 c + 0.5628 saturate(c / mean c)) by saturation.",
        &[
            ("value", ValueType::Float, scalar(0.0)),
            ("smoothness", ValueType::Float, scalar(0.05)),
            ("shallow", ValueType::Float3, white),
            ("deep", ValueType::Float3, white),
            ("fringe", ValueType::Float3, None),
            ("fringe_width", ValueType::Float, scalar(0.3)),
            ("saturation", ValueType::Float, scalar(1.0)),
        ],
        &[("shade", ValueType::Float3)],
        |b| {
            let value = b.function_input("value");
            let smoothness = b.function_input("smoothness");
            let shallow = b.function_input("shallow");
            let deep = b.function_input("deep");
            let fringe = b.function_input("fringe");
            let fringe_width = b.function_input("fringe_width");
            let saturation = b.function_input("saturation");
            let bands = node(
                b,
                "npr.bands",
                [
                    ("value", value.clone().into()),
                    ("low", (-THIRD).into()),
                    ("high", THIRD.into()),
                    ("smoothness", smoothness.into()),
                    ("lit_color", [1.0, 1.0, 1.0].into()),
                    ("shallow_color", shallow.into()),
                    ("deep_color", deep.into()),
                ],
            );
            let shade = b.out(bands, "color");
            let edges = node(
                b,
                "npr.bands",
                [
                    ("value", value.into()),
                    ("low", (-THIRD).into()),
                    ("high", THIRD.into()),
                    ("smoothness", fringe_width.into()),
                ],
            );
            let edge = b.out(edges, "upper_edge");
            let fringe = b.multiply(fringe, edge);
            let shade = b.add(shade, fringe);
            let shade = node(
                b,
                "color.saturated_shade",
                [("color", shade.into()), ("amount", saturation.into())],
            );
            let shade = b.out(shade, "value");
            b.function_output("shade", shade);
        },
    );
    matcap_group(b);
    group(
        b,
        SPECULAR_GROUP,
        "ZZZ's near-step toon GGX: GGX at roughness 1 - smoothness × glossiness with Fresnel from lerp(0.04, albedo, metallic), saturate(10 × D) for a hard-edged highlight, times the specular mask, color and strength.",
        &[
            ("normal", ValueType::Float3, None),
            ("albedo", ValueType::Float3, white),
            ("metallic", ValueType::Float, scalar(0.0)),
            ("smoothness", ValueType::Float, scalar(0.5)),
            ("spec", ValueType::Float, scalar(0.0)),
            ("color", ValueType::Float3, white),
            ("strength", ValueType::Float, scalar(1.0)),
            ("glossiness", ValueType::Float, scalar(1.0)),
        ],
        &[("color", ValueType::Float3)],
        |b| {
            let normal = b.function_input("normal");
            let albedo = b.function_input("albedo");
            let metallic = b.function_input("metallic");
            let smoothness = b.function_input("smoothness");
            let spec = b.function_input("spec");
            let color = b.function_input("color");
            let strength = b.function_input("strength");
            let glossiness = b.function_input("glossiness");
            let gloss = b.multiply(smoothness, glossiness);
            let roughness = one_minus(b, gloss);
            let f0 = b.lerp([0.04, 0.04, 0.04], albedo, metallic);
            let lobe = node(
                b,
                "npr.toon_ggx",
                [
                    ("normal", normal.into()),
                    ("roughness", roughness.into()),
                    ("f0", f0.into()),
                    ("gain", 10.0.into()),
                ],
            );
            let lobe = b.out(lobe, "value");
            let weight = b.multiply(spec, strength);
            let tinted = b.multiply(color, weight);
            let highlight = b.multiply(lobe, tinted);
            b.function_output("color", highlight);
        },
    );
    group(
        b,
        HAIR_BAND,
        "ZZZ's hair highlight: the painted band (mask) revealed where √HL(L·N') × HL(N'·H) is strong, N' being the normal bent onto a sphere around the head, so the highlight sweeps over the hair as one ring; times color, strength and the albedo by tint.",
        &[
            ("albedo", ValueType::Float3, white),
            ("mask", ValueType::Float, scalar(0.0)),
            ("softness", ValueType::Float, scalar(0.1)),
            ("color", ValueType::Float3, white),
            ("strength", ValueType::Float, scalar(0.5)),
            ("tint", ValueType::Float, scalar(0.5)),
        ],
        &[("color", ValueType::Float3)],
        |b| {
            let albedo = b.function_input("albedo");
            let mask = b.function_input("mask");
            let softness = b.function_input("softness");
            let color = b.function_input("color");
            let strength = b.function_input("strength");
            let tint = b.function_input("tint");
            let band = node(
                b,
                "npr.sphere_band",
                [
                    ("mask", mask.into()),
                    ("softness", softness.into()),
                    ("falloff", 20.0.into()),
                ],
            );
            let band = b.out(band, "band");
            let tinted = b.lerp([1.0, 1.0, 1.0], albedo, tint);
            let tinted = b.multiply(tinted, color);
            let tinted = b.multiply(tinted, strength);
            let highlight = b.multiply(tinted, band);
            b.function_output("color", highlight);
        },
    );
    group(
        b,
        DEPTH_RIM,
        "ZZZ's rim: where the scene depth width pixels out along the normal's screen direction lies behind (0…0.4 head-scaled metres), on the lit side (N·L 0…0.5), favouring upward-facing normals; times color × strength × 1.5.",
        &[
            ("normal", ValueType::Float3, None),
            ("color", ValueType::Float3, white),
            ("strength", ValueType::Float, scalar(0.5)),
            ("width", ValueType::Float, scalar(4.0)),
        ],
        &[("rim", ValueType::Float3)],
        |b| {
            let normal = b.function_input("normal");
            let color = b.function_input("color");
            let strength = b.function_input("strength");
            let width = b.function_input("width");
            let head = node(b, "npr.head", []);
            let scale = b.out(head, "scale");
            let mask = node(
                b,
                "npr.depth_rim",
                [
                    ("normal", normal.clone().into()),
                    ("width", width.into()),
                    ("direction", 1.0.into()),
                    ("scaling", 1.0.into()),
                    ("low", 0.0.into()),
                    ("high", 0.4.into()),
                    ("scale", scale.into()),
                ],
            );
            let mask = b.out(mask, "mask");
            let facing = node(
                b,
                "npr.wrap",
                [("normal", normal.clone().into()), ("wrap", 0.0.into())],
            );
            let facing = b.out(facing, "value");
            let side = node(
                b,
                "npr.light_side",
                [
                    ("facing", facing.into()),
                    ("low", 0.0.into()),
                    ("high", 0.5.into()),
                ],
            );
            let side = b.out(side, "value");
            let up = b.swizzle(normal, "y");
            let up = b.multiply(up, 0.5);
            let up = b.add(up, 0.5);
            let up = b.saturate(up);
            let up = b.lerp(1.0, up, 0.5);
            let rim = b.multiply(mask, side);
            let rim = b.multiply(rim, up);
            let rim = b.multiply(rim, strength);
            let rim = b.multiply(rim, 1.5);
            let rim = b.multiply(color, rim);
            b.function_output("rim", rim);
        },
    );
}

fn matcap_group(b: &mut Builder<'_>) {
    let scalar = |value: f32| Some(Constant::scalar(value));
    let white = Some(Constant::vec3(1.0, 1.0, 1.0));
    let mut inputs: Vec<(String, ValueType, Option<Constant>)> = vec![
        ("base".to_owned(), ValueType::Float3, white),
        ("normal".to_owned(), ValueType::Float3, None),
        ("id".to_owned(), ValueType::Float, scalar(0.0)),
        ("mask".to_owned(), ValueType::Float, scalar(1.0)),
    ];
    for slot in 1..=MATCAP_SLOTS {
        inputs.extend([
            (format!("matcap_{slot}"), ValueType::Texture, None),
            (format!("tint_{slot}"), ValueType::Float3, white),
            (format!("strength_{slot}"), ValueType::Float, scalar(1.0)),
            (format!("mode_{slot}"), ValueType::Float, scalar(2.0)),
        ]);
    }
    let inputs: Vec<(&str, ValueType, Option<Constant>)> = inputs
        .iter()
        .map(|(name, ty, default)| (name.as_str(), *ty, *default))
        .collect();
    group(
        b,
        MATCAPS,
        "ZZZ's gradient reflections: matcap N (read at the view-space normal) applies only to material ID N - 1, where a matcap is bound and mask allows, blended into base by tint × strength × its alpha with mode N (0 add, 1 multiply, 2 mix, 3 overlay, 4 screen).",
        &inputs,
        &[("color", ValueType::Float3)],
        |b| {
            let base = b.function_input("base");
            let normal = b.function_input("normal");
            let id = b.function_input("id");
            let mask = b.function_input("mask");
            let uv = node(
                b,
                "npr.matcap_uv",
                [
                    ("normal", normal.into()),
                    ("mode", 1.0.into()),
                    ("flip", 1.0.into()),
                ],
            );
            let uv = b.out(uv, "uv");
            let mut color = base;
            for slot in 1..=MATCAP_SLOTS {
                let texture = b.function_input(&format!("matcap_{slot}"));
                let tint = b.function_input(&format!("tint_{slot}"));
                let strength = b.function_input(&format!("strength_{slot}"));
                let mode = b.function_input(&format!("mode_{slot}"));
                let chosen = compare(b, id.clone(), (slot - 1) as f32, Compare::Equal);
                let bound = node(b, "texture.bound", [("texture", texture.clone().into())]);
                let bound = b.out(bound, "value");
                let weight = b.multiply(chosen, bound);
                let weight = b.multiply(weight, mask.clone());
                let blended = node(
                    b,
                    "npr.matcap",
                    [
                        ("base", color.into()),
                        ("texture", texture.into()),
                        ("uv", uv.clone().into()),
                        ("tint", tint.into()),
                        ("mask", weight.into()),
                        ("strength", strength.into()),
                        ("mode", mode.into()),
                    ],
                );
                color = b.out(blended, "color");
            }
            b.function_output("color", color);
        },
    );
}

struct Matcap {
    texture: Wire,
    tint: Wire,
    strength: Wire,
    mode: Wire,
}

fn matcaps(b: &mut Builder<'_>) -> Vec<Matcap> {
    (1..=MATCAP_SLOTS)
        .map(|slot| Matcap {
            texture: b.parameter(map(
                &format!("matcap_{slot}"),
                TextureKind::Colour,
                MATCAP_GROUP,
                &format!("Matcap {slot} (material {slot})"),
            )),
            tint: b.parameter(color(
                &format!("matcap_{slot}_color"),
                [1.0, 1.0, 1.0],
                MATCAP_GROUP,
                &format!("Matcap {slot} tint"),
            )),
            strength: b.parameter(float(
                &format!("matcap_{slot}_strength"),
                1.0,
                (0.0, 4.0),
                MATCAP_GROUP,
                &format!("Matcap {slot} strength"),
            )),
            mode: b.parameter(choice(
                &format!("matcap_{slot}_mode"),
                MATCAP_MODES,
                2,
                MATCAP_GROUP,
                &format!("Matcap {slot} blend"),
            )),
        })
        .collect()
}

struct Shading {
    smoothness: Wire,
    deep: Wire,
    fringe: Wire,
    fringe_width: Wire,
    saturation: Wire,
}

fn shading(b: &mut Builder<'_>, smoothness: f32) -> Shading {
    Shading {
        smoothness: b.parameter(float(
            "band_smoothness",
            smoothness,
            (0.0, 1.0),
            SHADE,
            "Band edge softness",
        )),
        deep: b.parameter(color(
            "deep_color",
            [0.58, 0.5, 0.66],
            SHADE,
            "Deep shadow colour",
        )),
        fringe: b.parameter(color(
            "fringe_color",
            [0.22, 0.05, 0.04],
            SHADE,
            "Terminator fringe colour",
        )),
        fringe_width: b.parameter(float(
            "fringe_width",
            0.3,
            (0.0, 1.0),
            SHADE,
            "Terminator fringe width",
        )),
        saturation: b.parameter(unit(
            "shade_saturation",
            1.0,
            SHADE,
            "Keep shade saturation",
        )),
    }
}

fn banded(b: &mut Builder<'_>, value: Wire, shallow: &Wire, shading: &Shading) -> Wire {
    let bands = call(
        b,
        BANDS,
        [
            ("value", value.into()),
            ("smoothness", shading.smoothness.clone().into()),
            ("shallow", shallow.into()),
            ("deep", shading.deep.clone().into()),
            ("fringe", shading.fringe.clone().into()),
            ("fringe_width", shading.fringe_width.clone().into()),
            ("saturation", shading.saturation.clone().into()),
        ],
    );
    b.out(bands, "shade")
}

struct Surface {
    normal: Wire,
    bias: Wire,
    outline: Wire,
    id: Wire,
    metallic: Wire,
    spec: Wire,
    smoothness: Wire,
    glow: Wire,
    matcap_mask: Wire,
}

fn build(library: &Library, part: Part) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    groups(&mut builder);
    let b = &mut builder;
    let imports = imports(b, &DEFAULTS);
    let ambient = ambient_strength(b);
    let controls = vertex_controls(b);
    let use_alpha = match part {
        Part::Face => alpha_use(b, "nose_line"),
        _ => b.constant(Constant::scalar(0.0)),
    };
    let base = base(b, &imports);
    let shading = shading(
        b,
        match part {
            Part::Face => 0.02,
            _ => 0.05,
        },
    );

    let maps = b.frame("Maps", |b| {
        let light = b.parameter(map(
            "light_texture",
            TextureKind::Data,
            MAPS_GROUP,
            match part {
                Part::Face => "Light map (_N: R shadow SDF, G cheek highlight, A chin occlusion)",
                _ => "Light map (_N: RG normal, B shadow bias, A outline width)",
            },
        ));
        let material = b.parameter(map(
            "material_texture",
            TextureKind::Data,
            MAPS_GROUP,
            "Material map (_M: R material ID, G metallic, B specular mask)",
        ));
        let mask = b.parameter(map(
            "mask_texture",
            TextureKind::Data,
            MAPS_GROUP,
            "Mask map (_A: G smoothness, B emission and matcap mask)",
        ));
        let face_light = light.clone();
        let split = call(
            b,
            MAPS,
            [
                ("light", light.into()),
                ("material", material.into()),
                ("mask", mask.into()),
                ("uv", base.uv.clone().into()),
                ("normal", base.normal.clone().into()),
                ("strength", imports.normal_scale.clone().into()),
            ],
        );
        let outline = match part {
            Part::Face => b.constant(Constant::scalar(1.0)),
            _ => b.out(split, "outline"),
        };
        let out = |name: &str| b.out(split, name);
        let surface = Surface {
            normal: match part {
                Part::Face => base.normal.clone(),
                _ => out("normal"),
            },
            bias: out("bias"),
            outline,
            id: out("id"),
            metallic: out("metallic"),
            spec: out("spec"),
            smoothness: out("smoothness"),
            glow: out("glow"),
            matcap_mask: out("matcap_mask"),
        };
        (surface, face_light)
    });
    let (surface, face_light) = maps;

    let albedo = match part {
        Part::Face => b.frame("Nose line", |b| {
            nose_line(
                b,
                &base.albedo,
                &base.alpha,
                &use_alpha,
                &imports.outline_color,
            )
        }),
        _ => base.albedo.clone(),
    };

    let key = b.frame("Bands, matcaps and highlights", |b| match part {
        Part::Face => face_shade(
            b,
            &face_light,
            &base.uv,
            &albedo,
            &imports.shade_color,
            &shading,
        ),
        _ => {
            let value = call(
                b,
                SHADOW_VALUE,
                [
                    ("normal", surface.normal.clone().into()),
                    ("bias", surface.bias.clone().into()),
                ],
            );
            let value = b.out(value, "value");
            let shade = banded(b, value, &imports.shade_color, &shading);
            let shaded = b.multiply(albedo.clone(), shade);
            let slots = matcaps(b);
            let mut inputs: Vec<(String, Arg)> = vec![
                ("base".to_owned(), shaded.into()),
                ("normal".to_owned(), surface.normal.clone().into()),
                ("id".to_owned(), surface.id.clone().into()),
                ("mask".to_owned(), surface.matcap_mask.clone().into()),
            ];
            for (index, slot) in slots.into_iter().enumerate() {
                let number = index + 1;
                inputs.extend([
                    (format!("matcap_{number}"), slot.texture.into()),
                    (format!("tint_{number}"), slot.tint.into()),
                    (format!("strength_{number}"), slot.strength.into()),
                    (format!("mode_{number}"), slot.mode.into()),
                ]);
            }
            let reflected = b.node(NodeKind::Call(MATCAPS.to_owned()));
            for (port, value) in inputs {
                b.set(reflected, &port, value);
            }
            let reflected = b.out(reflected, "color");
            let highlight = match part {
                Part::Hair => hair_band(b, &albedo, &surface.spec),
                _ => specular(b, &albedo, &surface),
            };
            b.add(reflected, highlight)
        }
    });

    let light = b.frame("Lights", |b| {
        let incoming = incoming(b);
        let rim_color = b.parameter(color("rim_color", [1.0, 1.0, 1.0], RIM, "Rim colour"));
        let rim_strength = b.parameter(float("rim_strength", 0.5, (0.0, 4.0), RIM, "Rim strength"));
        let rim_width = b.parameter(float("rim_width", 4.0, (0.0, 16.0), RIM, "Rim width (px)"));
        let rim = call(
            b,
            DEPTH_RIM,
            [
                ("normal", surface.normal.clone().into()),
                ("color", rim_color.into()),
                ("strength", rim_strength.into()),
                ("width", rim_width.into()),
            ],
        );
        let rim = b.out(rim, "rim");
        let lit = b.add(key, rim);
        b.multiply(lit, incoming.color)
    });

    let surface_shader = b.frame("Ambient and emission", |b| {
        let ambient = flat_ambient(b, &albedo, &ambient);
        let lighting = b.shader(
            NodeKind::Lighting,
            [("light", light.into()), ("ambient", ambient.into())],
        );
        let texture = b.texture(
            TextureKind::Colour,
            [("texture", imports.emission_texture.clone().into())],
        );
        let texture = b.out(texture, "rgb");
        let glowing = b.multiply(albedo.clone(), surface.glow.clone());
        let source = b.add(texture, glowing);
        let color = b.multiply(imports.emission_color.clone(), source);
        let emission = b.shader(NodeKind::Emission, [("color", color.into())]);
        b.shader(
            NodeKind::AddShader,
            [("a", lighting.into()), ("b", emission.into())],
        )
    });

    let alpha = b.lerp(base.alpha.clone(), 1.0, use_alpha.clone());

    let (outline_shader, shape) = b.frame("Outline", |b| {
        let colors: Vec<Wire> = (2..=5)
            .map(|index| {
                b.parameter(color(
                    &format!("outline_color_{index}"),
                    DEFAULTS.outline,
                    OUTLINE,
                    &format!("Outline colour, material {index}"),
                ))
            })
            .collect();
        let picked = node(
            b,
            "npr.per_id",
            [
                ("index", surface.id.clone().into()),
                ("v0", imports.outline_color.clone().into()),
                ("v1", colors[0].clone().into()),
                ("v2", colors[1].clone().into()),
                ("v3", colors[2].clone().into()),
                ("v4", colors[3].clone().into()),
                ("v5", colors[3].clone().into()),
                ("v6", colors[3].clone().into()),
                ("v7", colors[3].clone().into()),
            ],
        );
        let picked = b.out(picked, "value");
        let halved = b.multiply(picked, 0.5);
        let line = power(b, halved, 1.5);
        let line = b.multiply(line, albedo.clone());
        let facing = node(b, "npr.wrap", [("wrap", 1.0.into())]);
        let facing = b.out(facing, "value");
        let facing = b.saturate(facing);
        let shaded = b.lerp(0.01, 1.0, facing);
        let incoming = incoming(b);
        let lit = b.multiply(line.clone(), shaded);
        let lit = b.multiply(lit, incoming.color);
        let lit = b.multiply(lit, incoming.leading);
        let ambient = flat_ambient(b, &line, &ambient);
        let shader = b.shader(
            NodeKind::Lighting,
            [("light", lit.into()), ("ambient", ambient.into())],
        );
        let shape = outline_shape(
            b,
            &imports.outline_width,
            &surface.outline,
            &controls,
            1.0,
            &OutlineStyle {
                mode: 0.0,
                distances: [1.0, 3.0, 10.0],
                scales: [1.0, 1.0, 1.0],
                flatten_z: 0.0001,
                depth_mode: 1.0,
                depth_offset: 0.0,
            },
        );
        (shader, shape)
    });

    b.output([
        ("surface", surface_shader.into()),
        ("alpha", alpha.into()),
        ("outline", outline_shader.into()),
        ("outline_width", shape.width.into()),
        ("outline_direction", shape.direction.into()),
        ("outline_depth", shape.depth.into()),
    ]);
    builder.finish()
}

fn specular(b: &mut Builder<'_>, albedo: &Wire, surface: &Surface) -> Wire {
    let color = b.parameter(color(
        "specular_color",
        [1.0, 1.0, 1.0],
        SPECULAR,
        "Specular colour",
    ));
    let strength = b.parameter(float(
        "specular_strength",
        1.0,
        (0.0, 4.0),
        SPECULAR,
        "Specular strength",
    ));
    let glossiness = b.parameter(unit("glossiness", 1.0, SPECULAR, "Glossiness"));
    let highlight = call(
        b,
        SPECULAR_GROUP,
        [
            ("normal", surface.normal.clone().into()),
            ("albedo", albedo.into()),
            ("metallic", surface.metallic.clone().into()),
            ("smoothness", surface.smoothness.clone().into()),
            ("spec", surface.spec.clone().into()),
            ("color", color.into()),
            ("strength", strength.into()),
            ("glossiness", glossiness.into()),
        ],
    );
    b.out(highlight, "color")
}

fn hair_band(b: &mut Builder<'_>, albedo: &Wire, mask: &Wire) -> Wire {
    let softness = b.parameter(float(
        "hair_band_softness",
        0.1,
        (0.01, 1.0),
        HAIR_GROUP,
        "Highlight softness",
    ));
    let color = b.parameter(color(
        "hair_band_color",
        [1.0, 1.0, 1.0],
        HAIR_GROUP,
        "Highlight colour",
    ));
    let strength = b.parameter(float(
        "hair_band_strength",
        0.5,
        (0.0, 4.0),
        HAIR_GROUP,
        "Highlight strength",
    ));
    let band = call(
        b,
        HAIR_BAND,
        [
            ("albedo", albedo.into()),
            ("mask", mask.into()),
            ("softness", softness.into()),
            ("color", color.into()),
            ("strength", strength.into()),
        ],
    );
    b.out(band, "color")
}

fn nose_line(
    b: &mut Builder<'_>,
    albedo: &Wire,
    alpha: &Wire,
    use_alpha: &Wire,
    outline_color: &Wire,
) -> Wire {
    let normal = b.input(Input::Normal(rikka_shader_graph::DirectionSpace::World));
    let view = b.input(Input::ViewDirection(rikka_shader_graph::ViewSpace::World));
    let facing = b.dot(normal, view);
    let facing = b.saturate(facing);
    let facing = power(b, facing, 10.0);
    let near = b.parameter(unit("nose_line_start", 0.2, FACE, "Nose line fades in at"));
    let full = b.parameter(unit("nose_line_full", 0.6, FACE, "Nose line full at"));
    let shown = b.smoothstep(near, full, facing);
    let line = b.subtract(shown, alpha);
    let line = b.saturate(line);
    let line = b.multiply(line, use_alpha);
    let halved = b.multiply(outline_color, 0.5);
    let ink = power(b, halved, 2.1);
    let tint = b.lerp([1.0, 1.0, 1.0], ink, line);
    b.multiply(albedo, tint)
}

fn face_shade(
    b: &mut Builder<'_>,
    light: &Wire,
    uv: &Wire,
    albedo: &Wire,
    shallow: &Wire,
    shading: &Shading,
) -> Wire {
    let sample = node(
        b,
        "npr.face_sample",
        [("texture", light.into()), ("channel", 0.0.into())],
    );
    let (texel, bound) = optional_map(b, light, [1.0, 0.5, 0.5, 1.0], uv);
    let raw = [b.out(sample, "sdf"), b.out(sample, "mirrored")];
    let [sdf, mirrored] = raw.map(|value| {
        let value = b.multiply(value, 0.9);
        b.add(value, 0.1)
    });
    let mirror = b.parameter(choice(
        "face_mirror",
        &["left", "right"],
        0,
        FACE,
        "Mirror the SDF when the light is on the face's",
    ));
    let softness = b.parameter(float(
        "face_shadow_softness",
        0.5,
        (0.0, 1.0),
        FACE,
        "Face shadow softness",
    ));
    let shadow = call(
        b,
        FACE_SHADOW,
        [
            ("sdf", sdf.into()),
            ("mirrored", mirrored.into()),
            ("bound", bound.clone().into()),
            ("mirror", mirror.into()),
            ("softness", softness.into()),
        ],
    );
    let lit = b.out(shadow, "lit");
    let chin = b.swizzle(texel.clone(), "a");
    let lit = b.multiply(lit, chin);
    let cast = b.input(Input::Shadow);
    let lit = b.multiply(lit, cast);
    let value = b.multiply(lit, 2.0);
    let value = b.subtract(value, 1.0);
    let shade = banded(b, value, shallow, shading);
    let shaded = b.multiply(albedo, shade);
    let painted = b.swizzle(texel, "g");
    let painted = b.multiply(painted, bound);
    let strength = b.parameter(float(
        "face_highlight_strength",
        1.0,
        (0.0, 4.0),
        FACE,
        "Cheek highlight strength",
    ));
    let highlight = node(
        b,
        "npr.face_highlight",
        [("mask", painted.into()), ("threshold", 0.75.into())],
    );
    let highlight = b.out(highlight, "value");
    let highlight = b.multiply(highlight, strength);
    let highlight = b.multiply(highlight, albedo);
    b.add(shaded, highlight)
}
