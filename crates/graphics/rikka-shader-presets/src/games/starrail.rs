use rikka_shader_graph::{
    Arg, Builder, Constant, Error, Graph, GraphKind, Input, Library, NodeKind, ValueType, Wire,
};
use rikka_slang::TextureKind;

use crate::games::{
    Compare, Defaults, FACE, FACE_SHADOW, Imports, OutlineStyle, RIM, SHADE, SPECULAR, alpha_use,
    ambient_strength, base, call, choice, color, combine, common, compare, edge_ramp,
    face_shadow_group, flat_ambient, float, group, imports, incoming, map, node, one_minus,
    optional_map, outline_shape, toggle, unit, vertex_controls,
};
use crate::{Family, Preset};

const LIGHTMAP: &str = "Star Rail Lightmap";
const VALUES: &str = "Star Rail Material Values";
const RAMP_SHADE: &str = "Star Rail Ramp Shade";
const RAMP_PAIR: &str = "Star Rail Ramp Pair";
const SPECULAR_GROUP: &str = "Star Rail Specular";
const DEPTH_RIM: &str = "Star Rail Rim";
const STOCKINGS: &str = "Star Rail Stockings";
const FACE_SHADE: &str = "Star Rail Face Shade";

const RAMP: &str = "Ramp";
const LIGHTMAP_GROUP: &str = "Lightmap";
const VALUES_GROUP: &str = "Material values";
const RIM_SHADOW: &str = "Rim shadow";
const BLOOM: &str = "Bloom";
const STOCKINGS_GROUP: &str = "Stockings";
const EXPRESSIONS: &str = "Expressions";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Body,
    Hair,
    Face,
}

pub(super) fn presets() -> Vec<Preset> {
    vec![
        Preset {
            key: "starrail_body",
            title: "Honkai: Star Rail body",
            summary: "Shading taken wholly from warm and cool ramp pairs (two rows per material ID), painted stepped specular, a horizontal depth rim with rim shadow, per-ID values from the material values table, optional stockings and a bloom weight.",
            family: Family::Game,
            build: body,
        },
        Preset {
            key: "starrail_hair",
            title: "Honkai: Star Rail hair",
            summary: "The Star Rail body shading over a single-material warm and cool hair ramp, with the painted angel-ring highlight in the lightmap, depth rim and rim shadow.",
            family: Family::Game,
            build: hair,
        },
        Preset {
            key: "starrail_face",
            title: "Honkai: Star Rail face",
            summary: "SDF face shadow with eye shadow region, screen-space front-hair shadow, nose line shown near the frontal view, expression masks (blush, shyness, shadow) and a profile-only mouth outline.",
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

fn defaults(part: Part) -> Defaults {
    Defaults {
        shade: match part {
            Part::Face => [0.95, 0.78, 0.78],
            _ => [0.82, 0.7, 0.78],
        },
        outline: [0.24, 0.17, 0.18],
        outline_width: 0.0012,
    }
}

struct Values {
    specular_color: Wire,
    shininess: Wire,
    roughness: Wire,
    intensity: Wire,
    outline_color: Wire,
    rim_color: Wire,
    rim_softness: Wire,
    rim_dark: Wire,
    rim_shadow_color: Wire,
    rim_shadow_width: Wire,
    rim_shadow_feather: Wire,
    bloom_strength: Wire,
    bloom_color: Wire,
}

fn groups(b: &mut Builder<'_>) {
    common(b);
    face_shadow_group(b);
    let scalar = |value: f32| Some(Constant::scalar(value));
    let white = Some(Constant::vec3(1.0, 1.0, 1.0));
    group(
        b,
        LIGHTMAP,
        "Splits the Star Rail lightmap: spec = R (specular intensity and rim width mask), bias = G (shadow threshold, 0.5 neutral, below 0.05 always shadow, above 0.95 always lit), threshold = B (painted highlight shape, including the hair's angel ring), id = floor(8 × A) (material ID 0…7; 0 when single is on, for hair). Without a lightmap the texel is (0, 0.5, 0, 0). occlusion = vertex colour R when vertex_controls is on, else 1.",
        &[
            ("texture", ValueType::Texture, None),
            ("uv", ValueType::Float2, None),
            ("vertex_controls", ValueType::Float, scalar(0.0)),
            ("single", ValueType::Float, scalar(0.0)),
        ],
        &[
            ("spec", ValueType::Float),
            ("bias", ValueType::Float),
            ("threshold", ValueType::Float),
            ("id", ValueType::Float),
            ("occlusion", ValueType::Float),
        ],
        |b| {
            let texture = b.function_input("texture");
            let uv = b.function_input("uv");
            let controls = b.function_input("vertex_controls");
            let single = b.function_input("single");
            let (texel, _) = optional_map(b, &texture, [0.0, 0.5, 0.0, 0.0], &uv);
            let spec = b.swizzle(texel.clone(), "r");
            let bias = b.swizzle(texel.clone(), "g");
            let threshold = b.swizzle(texel.clone(), "b");
            let alpha = b.swizzle(texel, "a");
            let id = node(
                b,
                "npr.material_id",
                [
                    ("value", alpha.into()),
                    ("mode", 1.0.into()),
                    ("count", 8.0.into()),
                ],
            );
            let id = b.out(id, "index");
            let several = one_minus(b, single);
            let id = b.multiply(id, several);
            let vertex = b.input(Input::VertexColor);
            let red = b.swizzle(vertex, "r");
            let occlusion = b.lerp(1.0, red, controls);
            b.function_output("spec", spec);
            b.function_output("bias", bias);
            b.function_output("threshold", threshold);
            b.function_output("id", id);
            b.function_output("occlusion", occlusion);
        },
    );
    values_group(b);
    group(
        b,
        RAMP_PAIR,
        "One ramp of a warm/cool pair at u for material id: rows of rows counted from the bottom, two per ID; row_choice 0 reads the pair's first row, 1 its second, 2 the boundary between them (both rows averaged). Without a ramp texture: shade below the cel edge at u ≈ 0.53, white above it.",
        &[
            ("ramp", ValueType::Texture, None),
            ("u", ValueType::Float, scalar(0.5)),
            ("id", ValueType::Float, scalar(0.0)),
            ("rows", ValueType::Float, scalar(16.0)),
            ("row_choice", ValueType::Float, scalar(2.0)),
            ("shade", ValueType::Float3, white),
        ],
        &[("color", ValueType::Float3)],
        |b| {
            let ramp = b.function_input("ramp");
            let u = b.function_input("u");
            let id = b.function_input("id");
            let rows = b.function_input("rows");
            let choice = b.function_input("row_choice");
            let shade = b.function_input("shade");
            let second = compare(b, choice.clone(), 1.0, Compare::Equal);
            let between = compare(b, choice, 2.0, Compare::Equal);
            let blend = b.multiply(between, 0.5);
            let sampled = node(
                b,
                "npr.ramp",
                [
                    ("texture", ramp.clone().into()),
                    ("u", u.clone().into()),
                    ("row", id.into()),
                    ("rows", rows.into()),
                    ("group", 2.0.into()),
                    ("row_in_group", second.into()),
                    ("origin", 1.0.into()),
                    ("second_offset", 1.0.into()),
                    ("blend", blend.into()),
                ],
            );
            let sampled = b.out(sampled, "rgb");
            let bound = node(b, "texture.bound", [("texture", ramp.into())]);
            let bound = b.out(bound, "value");
            let edge = edge_ramp(b, u, [0.5, 0.56]);
            let fallback = b.lerp(shade, [1.0, 1.0, 1.0], edge);
            let color = b.lerp(fallback, sampled, bound);
            b.function_output("color", color);
        },
    );
    group(
        b,
        RAMP_SHADE,
        "Star Rail diffuse for the light being added: s = min(1, 2 × HL(N·L) × bias × occlusion) remapped to 0.25…1, pushed to 0.2 in cast shadow, 0 where the bias is below 0.05 and 1 above 0.95; the warm and cool ramps at u = s blend by warmth and multiply albedo on both sides (the lit colour is the ramp's right end).",
        &[
            ("normal", ValueType::Float3, None),
            ("albedo", ValueType::Float3, white),
            ("bias", ValueType::Float, scalar(0.5)),
            ("occlusion", ValueType::Float, scalar(1.0)),
            ("id", ValueType::Float, scalar(0.0)),
            ("rows", ValueType::Float, scalar(16.0)),
            ("row_choice", ValueType::Float, scalar(2.0)),
            ("warm", ValueType::Texture, None),
            ("cool", ValueType::Texture, None),
            ("warmth", ValueType::Float, scalar(1.0)),
            ("warm_shade", ValueType::Float3, white),
            ("cool_shade", ValueType::Float3, white),
        ],
        &[("color", ValueType::Float3), ("position", ValueType::Float)],
        |b| {
            let normal = b.function_input("normal");
            let albedo = b.function_input("albedo");
            let bias = b.function_input("bias");
            let occlusion = b.function_input("occlusion");
            let id = b.function_input("id");
            let rows = b.function_input("rows");
            let choice = b.function_input("row_choice");
            let warm = b.function_input("warm");
            let cool = b.function_input("cool");
            let warmth = b.function_input("warmth");
            let warm_shade = b.function_input("warm_shade");
            let cool_shade = b.function_input("cool_shade");
            let facing = node(
                b,
                "npr.wrap",
                [("normal", normal.into()), ("wrap", 0.0.into())],
            );
            let facing = b.out(facing, "value");
            let level = node(
                b,
                "npr.threshold",
                [
                    ("facing", facing.into()),
                    ("bias", bias.into()),
                    ("mode", 1.0.into()),
                    ("occlusion", occlusion.into()),
                ],
            );
            let value = b.out(level, "value");
            let lit_above = b.out(level, "force_lit");
            let shadow_below = b.out(level, "force_shadow");
            let lifted = b.multiply(value, 0.75);
            let lifted = b.add(lifted, 0.25);
            let cast = node(
                b,
                "npr.cast_gate",
                [("lit", lifted.into()), ("floor", 0.2.into())],
            );
            let cast = b.out(cast, "value");
            let position = b.lerp(cast, 1.0, lit_above);
            let position = b.lerp(position, 0.0, shadow_below);
            let mut ramp = |texture: Wire, shade: Wire| {
                let pair = call(
                    b,
                    RAMP_PAIR,
                    [
                        ("ramp", texture.into()),
                        ("u", position.clone().into()),
                        ("id", id.clone().into()),
                        ("rows", rows.clone().into()),
                        ("row_choice", choice.clone().into()),
                        ("shade", shade.into()),
                    ],
                );
                b.out(pair, "color")
            };
            let warm = ramp(warm, warm_shade);
            let cool = ramp(cool, cool_shade);
            let ramp = b.lerp(cool, warm, warmth);
            let color = node(
                b,
                "npr.shade_mix",
                [
                    ("albedo", albedo.into()),
                    ("shade", ramp.into()),
                    ("mode", 1.0.into()),
                ],
            );
            let color = b.out(color, "value");
            b.function_output("color", color);
            b.function_output("position", position);
        },
    );
    group(
        b,
        SPECULAR_GROUP,
        "Star Rail specular: smoothstep(1.03 - threshold ± roughness) of max(N·H, 0.01)^shininess × spec (lightmap R) × intensity, coloured by color × albedo and hidden in cast shadow; no metal or Fresnel term.",
        &[
            ("normal", ValueType::Float3, None),
            ("albedo", ValueType::Float3, white),
            ("spec", ValueType::Float, scalar(0.0)),
            ("threshold", ValueType::Float, scalar(0.0)),
            ("color", ValueType::Float3, white),
            ("shininess", ValueType::Float, scalar(10.0)),
            ("roughness", ValueType::Float, scalar(0.02)),
            ("intensity", ValueType::Float, scalar(1.0)),
        ],
        &[("color", ValueType::Float3)],
        |b| {
            let normal = b.function_input("normal");
            let albedo = b.function_input("albedo");
            let spec = b.function_input("spec");
            let threshold = b.function_input("threshold");
            let color = b.function_input("color");
            let shininess = b.function_input("shininess");
            let roughness = b.function_input("roughness");
            let intensity = b.function_input("intensity");
            let strength = b.multiply(spec, intensity);
            let stepped = node(
                b,
                "npr.stepped_specular",
                [
                    ("normal", normal.into()),
                    ("shininess", shininess.into()),
                    ("threshold", threshold.into()),
                    ("intensity", strength.into()),
                    ("softness", roughness.into()),
                    ("offset", 1.03.into()),
                ],
            );
            let value = b.out(stepped, "value");
            let shadow = b.input(Input::Shadow);
            let value = b.multiply(value, shadow);
            let tinted = b.multiply(color, albedo);
            let highlight = b.multiply(tinted, value);
            b.function_output("color", highlight);
        },
    );
    group(
        b,
        DEPTH_RIM,
        "Star Rail's horizontal depth rim: where the scene depth width × spec pixels sideways (outward from the silhouette, scaled by focal length and 1/√depth) lies 0.12…0.18 head-scaled metres behind, times (1 - N·V)^softness, color, and lerp(dark, 1, saturate(N·L × shadow)); spec (lightmap R) of 0 removes it.",
        &[
            ("normal", ValueType::Float3, None),
            ("spec", ValueType::Float, scalar(1.0)),
            ("color", ValueType::Float3, white),
            ("width", ValueType::Float, scalar(4.0)),
            ("softness", ValueType::Float, scalar(0.1)),
            ("dark", ValueType::Float, scalar(0.5)),
            ("strength", ValueType::Float, scalar(1.0)),
        ],
        &[("rim", ValueType::Float3)],
        |b| {
            let normal = b.function_input("normal");
            let spec = b.function_input("spec");
            let color = b.function_input("color");
            let width = b.function_input("width");
            let softness = b.function_input("softness");
            let dark = b.function_input("dark");
            let strength = b.function_input("strength");
            let head = node(b, "npr.head", []);
            let scale = b.out(head, "scale");
            let pixels = b.multiply(width, spec);
            let mask = node(
                b,
                "npr.depth_rim",
                [
                    ("normal", normal.clone().into()),
                    ("width", pixels.into()),
                    ("direction", 0.0.into()),
                    ("scaling", 2.0.into()),
                    ("low", 0.12.into()),
                    ("high", 0.18.into()),
                    ("scale", scale.into()),
                ],
            );
            let mask = b.out(mask, "mask");
            let edge = node(
                b,
                "npr.fresnel_rim",
                [
                    ("normal", normal.clone().into()),
                    ("power", softness.into()),
                ],
            );
            let edge = b.out(edge, "value");
            let facing = node(
                b,
                "npr.wrap",
                [("normal", normal.into()), ("wrap", 0.0.into())],
            );
            let facing = b.out(facing, "value");
            let shadow = b.input(Input::Shadow);
            let reach = b.multiply(facing, shadow);
            let reach = b.saturate(reach);
            let darkened = b.lerp(dark, 1.0, reach);
            let rim = b.multiply(mask, edge);
            let rim = b.multiply(rim, darkened);
            let rim = b.multiply(rim, strength);
            let rim = b.multiply(color, rim);
            b.function_output("rim", rim);
        },
    );
    group(
        b,
        STOCKINGS,
        "Stocking fabric over the albedo, blended in by enabled: range = the stocking map (R area and edge density, G sheen, B tiled pattern read at uv × tiling; white without a map); model 0 (denier) shows skin through thin fabric where the surface faces the view, model 1 (Star Rail) darkens the edges and adds a facing sheen of color.",
        &[
            ("albedo", ValueType::Float3, white),
            ("texture", ValueType::Texture, None),
            ("uv", ValueType::Float2, None),
            ("tiling", ValueType::Float, scalar(1.0)),
            ("enabled", ValueType::Float, scalar(0.0)),
            ("model", ValueType::Float, scalar(0.0)),
            ("color", ValueType::Float3, white),
            ("skin", ValueType::Float3, white),
            ("dark", ValueType::Float3, white),
            ("denier", ValueType::Float, scalar(0.35)),
            ("power", ValueType::Float, scalar(2.0)),
        ],
        &[("albedo", ValueType::Float3)],
        |b| {
            let albedo = b.function_input("albedo");
            let texture = b.function_input("texture");
            let uv = b.function_input("uv");
            let tiling = b.function_input("tiling");
            let enabled = b.function_input("enabled");
            let model = b.function_input("model");
            let color = b.function_input("color");
            let skin = b.function_input("skin");
            let dark = b.function_input("dark");
            let denier = b.function_input("denier");
            let power = b.function_input("power");
            let (range, _) = optional_map(b, &texture, [1.0, 1.0, 1.0, 1.0], &uv);
            let tiled = b.multiply(uv, tiling);
            let (pattern, _) = optional_map(b, &texture, [1.0, 1.0, 1.0, 1.0], &tiled);
            let area = b.swizzle(range.clone(), "r");
            let sheen = b.swizzle(range, "g");
            let pattern = b.swizzle(pattern, "b");
            let range = combine(b, area, sheen, pattern);
            let fabric = node(
                b,
                "npr.stocking",
                [
                    ("mode", model.into()),
                    ("albedo", albedo.clone().into()),
                    ("range", range.into()),
                    ("color", color.into()),
                    ("skin_color", skin.into()),
                    ("dark_color", dark.into()),
                    ("denier", denier.into()),
                    ("power", power.into()),
                    ("sheen_power", 1.0.into()),
                    ("dark_width", 0.5.into()),
                    ("roughness", 0.0.into()),
                ],
            );
            let fabric = b.out(fabric, "color");
            let dressed = b.lerp(albedo, fabric, enabled);
            b.function_output("albedo", dressed);
        },
    );
    group(
        b,
        FACE_SHADE,
        "Star Rail face shade for the light being added: shade (face shadow colour) where the SDF face shadow leaves the face unlit, eye_shade over the eye-shadow region (eye_mask, lit from HL(F·L) 0.3…0.5), the eyes themselves lit by eyes_lit, and the front hair's screen-space shadow tinted by hair_shade; all fade with the cast shadow.",
        &[
            ("lit", ValueType::Float, scalar(1.0)),
            ("facing", ValueType::Float, scalar(1.0)),
            ("eye_mask", ValueType::Float, scalar(0.0)),
            ("shade", ValueType::Float3, white),
            ("eye_shade", ValueType::Float3, white),
            ("eyes_lit", ValueType::Float, scalar(0.0)),
            ("hair_shade", ValueType::Float3, white),
            ("hair_distance", ValueType::Float, scalar(0.03)),
        ],
        &[("shade", ValueType::Float3)],
        |b| {
            let lit = b.function_input("lit");
            let facing = b.function_input("facing");
            let eye_mask = b.function_input("eye_mask");
            let shade = b.function_input("shade");
            let eye_shade = b.function_input("eye_shade");
            let eyes_lit = b.function_input("eyes_lit");
            let hair_shade = b.function_input("hair_shade");
            let hair_distance = b.function_input("hair_distance");
            let lit = node(
                b,
                "npr.cast_gate",
                [("lit", lit.into()), ("floor", 0.0.into())],
            );
            let lit = b.out(lit, "value");
            let face = b.lerp(shade, [1.0, 1.0, 1.0], lit);
            let eye_lit = b.smoothstep(0.3, 0.5, facing);
            let eye_lit = node(
                b,
                "npr.cast_gate",
                [("lit", eye_lit.into()), ("floor", 0.0.into())],
            );
            let eye_lit = b.out(eye_lit, "value");
            let eye = b.lerp(eye_shade, [1.0, 1.0, 1.0], eye_lit);
            let shaded = b.lerp(face, eye, eye_mask.clone());
            let inside = compare(b, eye_mask.clone(), 0.1, Compare::Greater);
            let beyond = compare(b, eye_mask, 0.8, Compare::Greater);
            let eyes = b.subtract(inside, beyond);
            let eyes = b.multiply(eyes, eyes_lit);
            let shaded = b.lerp(shaded, [1.0, 1.0, 1.0], eyes);
            let hair = node(b, "npr.hair_shadow", [("distance", hair_distance.into())]);
            let hair = b.out(hair, "lit");
            let hair = b.lerp(hair_shade, [1.0, 1.0, 1.0], hair);
            let shaded = b.multiply(shaded, hair);
            b.function_output("shade", shaded);
        },
    );
}

fn values_group(b: &mut Builder<'_>) {
    let scalar = |value: f32| Some(Constant::scalar(value));
    let white = Some(Constant::vec3(1.0, 1.0, 1.0));
    group(
        b,
        VALUES,
        "Per-material values from Star Rail's 8×8 material values table (column = material ID; rows: 0 specular colour, 1 shininess / roughness / intensity, 2 outline colour, 3 rim colour, 4 rim type / softness / dark, 5 rim shadow colour, 6 rim shadow width / feather / bloom intensity, 7 bloom colour) when one is bound, the specular row scaled by 10 / 2 / 2 as HoyoToon reads it; else the inputs.",
        &[
            ("table", ValueType::Texture, None),
            ("id", ValueType::Float, scalar(0.0)),
            ("specular_color", ValueType::Float3, white),
            ("shininess", ValueType::Float, scalar(10.0)),
            ("roughness", ValueType::Float, scalar(0.02)),
            ("intensity", ValueType::Float, scalar(1.0)),
            ("outline_color", ValueType::Float3, white),
            ("rim_color", ValueType::Float3, white),
            ("rim_softness", ValueType::Float, scalar(0.1)),
            ("rim_dark", ValueType::Float, scalar(0.5)),
            ("rim_shadow_color", ValueType::Float3, white),
            ("rim_shadow_width", ValueType::Float, scalar(1.0)),
            ("rim_shadow_feather", ValueType::Float, scalar(0.01)),
            ("bloom_strength", ValueType::Float, scalar(0.0)),
            ("bloom_color", ValueType::Float3, white),
        ],
        &[
            ("specular_color", ValueType::Float3),
            ("shininess", ValueType::Float),
            ("roughness", ValueType::Float),
            ("intensity", ValueType::Float),
            ("outline_color", ValueType::Float3),
            ("rim_color", ValueType::Float3),
            ("rim_softness", ValueType::Float),
            ("rim_dark", ValueType::Float),
            ("rim_shadow_color", ValueType::Float3),
            ("rim_shadow_width", ValueType::Float),
            ("rim_shadow_feather", ValueType::Float),
            ("bloom_strength", ValueType::Float),
            ("bloom_color", ValueType::Float3),
        ],
        |b| {
            let table = b.function_input("table");
            let id = b.function_input("id");
            let bound = node(b, "texture.bound", [("texture", table.clone().into())]);
            let bound = b.out(bound, "value");
            let rows: Vec<Wire> = (0..8)
                .map(|row| {
                    let texel = node(
                        b,
                        "npr.param_lut",
                        [
                            ("texture", table.clone().into()),
                            ("column", id.clone().into()),
                            ("row", (row as f32).into()),
                        ],
                    );
                    b.out(texel, "value")
                })
                .collect();
            let mut pick = |name: &str, row: usize, swizzle: &str, scale: f32| {
                let fallback = b.function_input(name);
                let read = b.swizzle(rows[row].clone(), swizzle);
                let read = b.multiply(read, scale);
                let value = b.lerp(fallback, read, bound.clone());
                b.function_output(name, value);
            };
            pick("specular_color", 0, "rgb", 1.0);
            pick("shininess", 1, "r", 10.0);
            pick("roughness", 1, "g", 2.0);
            pick("intensity", 1, "b", 2.0);
            pick("outline_color", 2, "rgb", 1.0);
            pick("rim_color", 3, "rgb", 1.0);
            pick("rim_softness", 4, "g", 1.0);
            pick("rim_dark", 4, "b", 1.0);
            pick("rim_shadow_color", 5, "rgb", 1.0);
            pick("rim_shadow_width", 6, "r", 1.0);
            pick("rim_shadow_feather", 6, "g", 1.0);
            pick("bloom_strength", 6, "b", 1.0);
            pick("bloom_color", 7, "rgb", 1.0);
        },
    );
}

fn values(b: &mut Builder<'_>, imports: &Imports, id: &Wire, uses_rim_shadow: bool) -> Values {
    let table = b.parameter(map(
        "params_lut",
        TextureKind::Data,
        VALUES_GROUP,
        "Material values table (8×8)",
    ));
    let specular_color = b.parameter(color(
        "specular_color",
        [1.0, 1.0, 1.0],
        SPECULAR,
        "Specular colour",
    ));
    let shininess = b.parameter(float(
        "specular_shininess",
        10.0,
        (0.1, 500.0),
        SPECULAR,
        "Specular shininess",
    ));
    let roughness = b.parameter(float(
        "specular_roughness",
        0.02,
        (0.0, 1.0),
        SPECULAR,
        "Specular roughness",
    ));
    let intensity = b.parameter(float(
        "specular_intensity",
        1.0,
        (0.0, 50.0),
        SPECULAR,
        "Specular intensity",
    ));
    let rim_color = b.parameter(color("rim_color", [1.0, 1.0, 1.0], RIM, "Rim colour"));
    let rim_softness = b.parameter(float(
        "rim_softness",
        0.1,
        (0.01, 0.9),
        RIM,
        "Rim edge softness",
    ));
    let rim_dark = b.parameter(unit("rim_dark", 0.5, RIM, "Rim in shadow"));
    let bloom_strength = b.parameter(float(
        "bloom_strength",
        0.0,
        (0.0, 4.0),
        BLOOM,
        "Bloom strength",
    ));
    let bloom_color = b.parameter(color("bloom_color", [1.0, 1.0, 1.0], BLOOM, "Bloom colour"));
    let mut inputs: Vec<(&str, Arg)> = vec![
        ("table", table.into()),
        ("id", id.into()),
        ("specular_color", specular_color.into()),
        ("shininess", shininess.into()),
        ("roughness", roughness.into()),
        ("intensity", intensity.into()),
        ("outline_color", imports.outline_color.clone().into()),
        ("rim_color", rim_color.into()),
        ("rim_softness", rim_softness.into()),
        ("rim_dark", rim_dark.into()),
        ("bloom_strength", bloom_strength.into()),
        ("bloom_color", bloom_color.into()),
    ];
    if uses_rim_shadow {
        inputs.extend([
            (
                "rim_shadow_color",
                b.parameter(color(
                    "rim_shadow_color",
                    [0.75, 0.72, 0.85],
                    RIM_SHADOW,
                    "Rim shadow colour",
                ))
                .into(),
            ),
            (
                "rim_shadow_width",
                b.parameter(float(
                    "rim_shadow_width",
                    1.0,
                    (0.0, 4.0),
                    RIM_SHADOW,
                    "Rim shadow width",
                ))
                .into(),
            ),
            (
                "rim_shadow_feather",
                b.parameter(unit(
                    "rim_shadow_feather",
                    0.01,
                    RIM_SHADOW,
                    "Rim shadow feather",
                ))
                .into(),
            ),
        ]);
    }
    let read = b.node(NodeKind::Call(VALUES.to_owned()));
    for (port, value) in inputs {
        b.set(read, port, value);
    }
    let out = |name: &str| b.out(read, name);
    Values {
        specular_color: out("specular_color"),
        shininess: out("shininess"),
        roughness: out("roughness"),
        intensity: out("intensity"),
        outline_color: out("outline_color"),
        rim_color: out("rim_color"),
        rim_softness: out("rim_softness"),
        rim_dark: out("rim_dark"),
        rim_shadow_color: out("rim_shadow_color"),
        rim_shadow_width: out("rim_shadow_width"),
        rim_shadow_feather: out("rim_shadow_feather"),
        bloom_strength: out("bloom_strength"),
        bloom_color: out("bloom_color"),
    }
}

fn build(library: &Library, part: Part) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    groups(&mut builder);
    let b = &mut builder;
    let defaults = defaults(part);
    let imports = imports(b, &defaults);
    let ambient = ambient_strength(b);
    let controls = vertex_controls(b);
    let use_alpha = alpha_use(b, "emission");

    let (surface, lightmap) = b.frame("Surface and lightmap", |b| {
        let surface = base(b, &imports);
        let texture = b.parameter(map(
            "lightmap_texture",
            TextureKind::Data,
            LIGHTMAP_GROUP,
            "Lightmap (R specular and rim, G shadow, B highlight, A material ID)",
        ));
        let single = match part {
            Part::Hair => 1.0,
            _ => 0.0,
        };
        let split = call(
            b,
            LIGHTMAP,
            [
                ("texture", texture.into()),
                ("uv", surface.uv.clone().into()),
                ("vertex_controls", controls.clone().into()),
                ("single", single.into()),
            ],
        );
        (surface, split)
    });
    let spec = b.out(lightmap, "spec");
    let id = b.out(lightmap, "id");
    let bias = b.out(lightmap, "bias");
    let occlusion = b.out(lightmap, "occlusion");
    let threshold = b.out(lightmap, "threshold");

    let values = b.frame("Material values", |b| {
        values(b, &imports, &id, part != Part::Face)
    });

    let albedo = b.frame("Albedo dressing", |b| match part {
        Part::Face => face_albedo(b, &surface.albedo, &surface.uv),
        Part::Body => {
            let dressed = stockings(b, &surface.albedo, &surface.uv);
            rim_shadow(b, &dressed, &values)
        }
        Part::Hair => rim_shadow(b, &surface.albedo, &values),
    });

    let key = b.frame("Diffuse and specular", |b| match part {
        Part::Face => face_shade(b, &imports, &albedo, &surface.uv),
        _ => {
            let warm = b.parameter(map(
                "ramp_warm_texture",
                TextureKind::Ramp,
                RAMP,
                "Warm ramp",
            ));
            let cool = b.parameter(map(
                "ramp_cool_texture",
                TextureKind::Ramp,
                RAMP,
                "Cool ramp",
            ));
            let warmth = b.parameter(unit("warmth", 1.0, RAMP, "Warmth (0 cool ramp, 1 warm)"));
            let row_choice = b.parameter(choice(
                "ramp_row",
                &["first", "second", "between"],
                2,
                RAMP,
                "Row of each material's pair",
            ));
            let cool_shade = b.parameter(color(
                "cool_shade_color",
                [0.64, 0.66, 0.84],
                SHADE,
                "Cool shade colour",
            ));
            let rows = match part {
                Part::Hair => 2.0,
                _ => 16.0,
            };
            let shaded = call(
                b,
                RAMP_SHADE,
                [
                    ("normal", surface.normal.clone().into()),
                    ("albedo", albedo.clone().into()),
                    ("bias", bias.into()),
                    ("occlusion", occlusion.into()),
                    ("id", id.clone().into()),
                    ("rows", rows.into()),
                    ("row_choice", row_choice.into()),
                    ("warm", warm.into()),
                    ("cool", cool.into()),
                    ("warmth", warmth.into()),
                    ("warm_shade", imports.shade_color.clone().into()),
                    ("cool_shade", cool_shade.into()),
                ],
            );
            let diffuse = b.out(shaded, "color");
            let highlight = call(
                b,
                SPECULAR_GROUP,
                [
                    ("normal", surface.normal.clone().into()),
                    ("albedo", albedo.clone().into()),
                    ("spec", spec.clone().into()),
                    ("threshold", threshold.into()),
                    ("color", values.specular_color.clone().into()),
                    ("shininess", values.shininess.clone().into()),
                    ("roughness", values.roughness.clone().into()),
                    ("intensity", values.intensity.clone().into()),
                ],
            );
            let highlight = b.out(highlight, "color");
            b.add(diffuse, highlight)
        }
    });

    let (boost, emission) = b.frame("Emission and bloom", |b| {
        let texture = b.texture(
            TextureKind::Colour,
            [("texture", imports.emission_texture.clone().into())],
        );
        let texture = b.out(texture, "rgb");
        let source = b.lerp(texture, surface.albedo.clone(), use_alpha.clone());
        let mask = b.lerp(1.0, surface.alpha.clone(), use_alpha.clone());
        let glow = node(
            b,
            "npr.emission",
            [
                ("albedo", source.into()),
                ("mask", mask.into()),
                ("threshold", 0.5.into()),
                ("color", imports.emission_color.clone().into()),
                ("strength", 1.0.into()),
                ("bloom_color", values.bloom_color.clone().into()),
                ("bloom_strength", values.bloom_strength.clone().into()),
            ],
        );
        let boost = b.out(glow, "boost");
        let emission = b.out(glow, "emission");
        let emission = b.multiply(emission, boost.clone());
        let emission = b.shader(NodeKind::Emission, [("color", emission.into())]);
        (boost, emission)
    });

    let light = b.frame("Lights", |b| {
        let incoming = incoming(b);
        let width = b.parameter(float("rim_width", 4.0, (0.0, 16.0), RIM, "Rim width (px)"));
        let strength = b.parameter(float("rim_strength", 1.0, (0.0, 4.0), RIM, "Rim strength"));
        let rim = call(
            b,
            DEPTH_RIM,
            [
                ("normal", surface.normal.clone().into()),
                ("spec", spec.clone().into()),
                ("color", values.rim_color.clone().into()),
                ("width", width.into()),
                ("softness", values.rim_softness.clone().into()),
                ("dark", values.rim_dark.clone().into()),
                ("strength", strength.into()),
            ],
        );
        let rim = b.out(rim, "rim");
        let rim = match part {
            Part::Face => rim,
            _ => {
                let rim_mask = compare(b, spec.clone(), 0.0, Compare::Greater);
                b.multiply(rim, rim_mask)
            }
        };
        let lit = b.add(key.clone(), rim);
        let lit = b.multiply(lit, incoming.color.clone());
        let shadow = b.input(Input::Shadow);
        let extra = b.multiply(albedo.clone(), incoming.color);
        let extra = b.multiply(extra, shadow);
        let light = b.lerp(extra, lit, incoming.leading);
        b.multiply(light, boost.clone())
    });

    let surface_shader = b.frame("Ambient", |b| {
        let ambient = flat_ambient(b, &albedo, &ambient);
        let ambient = b.multiply(ambient, boost.clone());
        let lighting = b.shader(
            NodeKind::Lighting,
            [("light", light.into()), ("ambient", ambient.into())],
        );
        b.shader(
            NodeKind::AddShader,
            [("a", lighting.into()), ("b", emission.into())],
        )
    });

    let alpha = b.lerp(surface.alpha.clone(), 1.0, use_alpha);

    let (outline_shader, shape) = b.frame("Outline", |b| {
        let incoming = incoming(b);
        let color = values.outline_color.clone();
        let lit = b.multiply(color.clone(), incoming.color);
        let lit = b.multiply(lit, incoming.leading);
        let ambient = flat_ambient(b, &color, &ambient);
        let shader = b.shader(
            NodeKind::Lighting,
            [("light", lit.into()), ("ambient", ambient.into())],
        );
        let multiplier = match part {
            Part::Face => {
                let vertex = b.input(Input::VertexColor);
                let blue = b.swizzle(vertex, "b");
                let mask = b.multiply(blue, controls.clone());
                let fade = node(b, "npr.outline_fade", [("mask", mask.into())]);
                b.out(fade, "value")
            }
            _ => b.constant(Constant::scalar(1.0)),
        };
        let shape = outline_shape(
            b,
            &imports.outline_width,
            &multiplier,
            &controls,
            4.0,
            &OutlineStyle {
                mode: 2.0,
                distances: [1.6, 4.0, 100.0],
                scales: [1.0, 2.5, 2.5],
                flatten_z: -0.1,
                depth_mode: 0.0,
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

fn rim_shadow(b: &mut Builder<'_>, albedo: &Wire, values: &Values) -> Wire {
    let strength = b.parameter(float(
        "rim_shadow_strength",
        1.0,
        (0.0, 4.0),
        RIM_SHADOW,
        "Rim shadow strength",
    ));
    let darkening = node(
        b,
        "npr.rim_shadow",
        [
            ("power", 1.0.into()),
            ("width", values.rim_shadow_width.clone().into()),
            ("feather", values.rim_shadow_feather.clone().into()),
            ("intensity", strength.into()),
            ("color", values.rim_shadow_color.clone().into()),
        ],
    );
    let darkening = b.out(darkening, "value");
    b.multiply(albedo, darkening)
}

fn stockings(b: &mut Builder<'_>, albedo: &Wire, uv: &Wire) -> Wire {
    let enabled = b.parameter(toggle("stockings", false, STOCKINGS_GROUP, "Stockings"));
    let texture = b.parameter(map(
        "stocking_texture",
        TextureKind::Data,
        STOCKINGS_GROUP,
        "Stocking map (R area, G sheen, B tiled pattern)",
    ));
    let model = b.parameter(choice(
        "stocking_model",
        &["denier", "star_rail"],
        0,
        STOCKINGS_GROUP,
        "Stocking model",
    ));
    let fabric = b.parameter(color(
        "stocking_color",
        [1.0, 1.0, 1.0],
        STOCKINGS_GROUP,
        "Fabric colour",
    ));
    let skin = b.parameter(color(
        "stocking_skin_color",
        [0.98, 0.8, 0.72],
        STOCKINGS_GROUP,
        "Skin under the fabric",
    ));
    let dark = b.parameter(color(
        "stocking_dark_color",
        [0.5, 0.45, 0.45],
        STOCKINGS_GROUP,
        "Fabric edge colour",
    ));
    let denier = b.parameter(unit(
        "stocking_denier",
        0.35,
        STOCKINGS_GROUP,
        "Fabric density facing the view",
    ));
    let power = b.parameter(float(
        "stocking_power",
        2.0,
        (0.04, 16.0),
        STOCKINGS_GROUP,
        "Stocking falloff",
    ));
    let tiling = b.parameter(float(
        "stocking_tiling",
        1.0,
        (0.1, 64.0),
        STOCKINGS_GROUP,
        "Pattern tiling",
    ));
    let dressed = call(
        b,
        STOCKINGS,
        [
            ("albedo", albedo.into()),
            ("texture", texture.into()),
            ("uv", uv.into()),
            ("tiling", tiling.into()),
            ("enabled", enabled.into()),
            ("model", model.into()),
            ("color", fabric.into()),
            ("skin", skin.into()),
            ("dark", dark.into()),
            ("denier", denier.into()),
            ("power", power.into()),
        ],
    );
    b.out(dressed, "albedo")
}

fn face_albedo(b: &mut Builder<'_>, albedo: &Wire, uv: &Wire) -> Wire {
    let face_map = b.parameter(map(
        "face_map",
        TextureKind::Data,
        FACE,
        "Face map (R eye shadow, G eye mask, B nose line, A shadow SDF)",
    ));
    let (texel, _) = optional_map(b, &face_map, [0.0, 0.0, 0.0, 0.5], uv);
    let nose_mask = b.swizzle(texel, "b");
    let power = b.parameter(float(
        "nose_line_power",
        1.0,
        (0.0, 8.0),
        FACE,
        "Nose line power",
    ));
    let nose = node(
        b,
        "npr.nose_line",
        [
            ("mask", nose_mask.into()),
            ("mode", 0.0.into()),
            ("power", power.into()),
            ("threshold", 1.03.into()),
        ],
    );
    let nose = b.out(nose, "value");
    let nose_color = b.parameter(color(
        "nose_line_color",
        [0.75, 0.5, 0.48],
        FACE,
        "Nose line colour",
    ));
    let darkened = b.multiply(albedo, nose_color);
    let albedo = b.lerp(albedo, darkened, nose);
    let expressions = b.parameter(map(
        "expression_texture",
        TextureKind::Data,
        EXPRESSIONS,
        "Expression map (R blush, G shyness, B shadow)",
    ));
    let (masks, _) = optional_map(b, &expressions, [0.0, 0.0, 0.0, 0.0], uv);
    let masks = b.swizzle(masks, "rgb");
    let blush = b.parameter(color(
        "blush_color",
        [1.0, 0.72, 0.72],
        EXPRESSIONS,
        "Blush colour",
    ));
    let shy = b.parameter(color(
        "shy_color",
        [1.0, 0.8, 0.8],
        EXPRESSIONS,
        "Shyness colour",
    ));
    let shadow = b.parameter(color(
        "expression_shadow_color",
        [0.7, 0.68, 0.8],
        EXPRESSIONS,
        "Expression shadow colour",
    ));
    let strengths = ["blush", "shy", "expression_shadow"].map(|name| {
        b.parameter(unit(
            &format!("{name}_strength"),
            0.0,
            EXPRESSIONS,
            &format!("{} strength", name.replace('_', " ")),
        ))
    });
    let [blush_strength, shy_strength, shadow_strength] = strengths;
    let strength = combine(b, blush_strength, shy_strength, shadow_strength);
    let tinted = node(
        b,
        "npr.expression",
        [
            ("color", albedo.into()),
            ("mask", masks.into()),
            ("red", blush.into()),
            ("green", shy.into()),
            ("blue", shadow.into()),
            ("strength", strength.into()),
        ],
    );
    b.out(tinted, "value")
}

fn face_shade(b: &mut Builder<'_>, imports: &Imports, albedo: &Wire, uv: &Wire) -> Wire {
    let face_map = b.parameter(map(
        "face_map",
        TextureKind::Data,
        FACE,
        "Face map (R eye shadow, G eye mask, B nose line, A shadow SDF)",
    ));
    let sample = node(
        b,
        "npr.face_sample",
        [
            ("texture", face_map.clone().into()),
            ("channel", 3.0.into()),
        ],
    );
    let (texel, bound) = optional_map(b, &face_map, [0.0, 0.0, 0.0, 0.5], uv);
    let eye_mask = b.swizzle(texel, "r");
    let mirror = b.parameter(choice(
        "face_mirror",
        &["left", "right"],
        1,
        FACE,
        "Mirror the SDF when the light is on the face's",
    ));
    let softness = b.parameter(float(
        "face_shadow_softness",
        0.01,
        (0.0, 0.2),
        FACE,
        "Face shadow softness",
    ));
    let sdf = b.out(sample, "sdf");
    let mirrored = b.out(sample, "mirrored");
    let shadow = call(
        b,
        FACE_SHADOW,
        [
            ("sdf", sdf.into()),
            ("mirrored", mirrored.into()),
            ("bound", bound.into()),
            ("mirror", mirror.into()),
            ("softness", softness.into()),
        ],
    );
    let lit = b.out(shadow, "lit");
    let facing = b.out(shadow, "facing");
    let eye_shade = b.parameter(color(
        "eye_shadow_color",
        [0.85, 0.72, 0.75],
        FACE,
        "Eye shadow colour",
    ));
    let eyes_lit = b.parameter(unit("eyes_lit", 0.0, FACE, "Eyes always lit"));
    let hair_shade = b.parameter(color(
        "hair_shadow_color",
        [0.82, 0.66, 0.68],
        FACE,
        "Hair shadow colour",
    ));
    let hair_distance = b.parameter(float(
        "hair_shadow_distance",
        0.03,
        (0.0, 0.2),
        FACE,
        "Hair shadow distance (m)",
    ));
    let shade = call(
        b,
        FACE_SHADE,
        [
            ("lit", lit.into()),
            ("facing", facing.into()),
            ("eye_mask", eye_mask.into()),
            ("shade", imports.shade_color.clone().into()),
            ("eye_shade", eye_shade.into()),
            ("eyes_lit", eyes_lit.into()),
            ("hair_shade", hair_shade.into()),
            ("hair_distance", hair_distance.into()),
        ],
    );
    let shade = b.out(shade, "shade");
    b.multiply(albedo, shade)
}
