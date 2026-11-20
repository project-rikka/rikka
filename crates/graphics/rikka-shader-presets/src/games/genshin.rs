use rikka_shader_graph::{
    Builder, Constant, Error, Graph, GraphKind, Input, Library, NodeKind, ValueType, Wire,
};
use rikka_slang::TextureKind;

use crate::games::{
    Compare, Defaults, FACE, FACE_SHADOW, Imports, OUTLINE, OutlineShape, OutlineStyle, RIM, SHADE,
    SPECULAR, alpha_use, ambient_strength, base, call, choice, color, common, compare, edge_ramp,
    face_shadow_group, flat_ambient, float, group, imports, incoming, integer, map, maximum, node,
    one_minus, optional_map, outline_shape, unit, vertex_controls,
};

use crate::{Family, Preset};

const LIGHTMAP: &str = "Genshin Lightmap";
const RAMP_SHADE: &str = "Genshin Ramp Shade";
const SPECULAR_GROUP: &str = "Genshin Specular";
const METAL: &str = "Genshin Metal";
const DEPTH_RIM: &str = "Genshin Rim";
const RAMP_COLOR: &str = "Genshin Ramp Color";

const RAMP: &str = "Ramp";
const LIGHTMAP_GROUP: &str = "Lightmap";
const METAL_GROUP: &str = "Metal";

const RAMP_ROWS: f32 = 10.0;
const NIGHT_ROWS: f32 = 5.0;
const LIGHT_AREA: f32 = 0.55;

pub(super) fn presets() -> Vec<Preset> {
    vec![
        Preset {
            key: "genshin_body",
            title: "Genshin Impact body",
            summary: "Lightmap-driven cel shading for cloth, skin and hair: the ramp colours only the shadow side, day and night ramp rows per material ID, painted stepped specular with metal matcap, a horizontal depth rim and outlines coloured per material ID.",
            family: Family::Game,
            build: body,
        },
        Preset {
            key: "genshin_face",
            title: "Genshin Impact face",
            summary: "SDF face shadow from the head-plane light, mirrored by the light's side, shaded with the skin row of the ramp, with blush, nose line, depth rim and outline; no specular.",
            family: Family::Game,
            build: face,
        },
    ]
}

const DEFAULTS: Defaults = Defaults {
    shade: [0.86, 0.72, 0.74],
    outline: [0.22, 0.16, 0.16],
    outline_width: 0.0012,
};

struct Settings {
    ramp: Wire,
    day: Wire,
    night_shade: Wire,
    ambient: Wire,
    vertex_controls: Wire,
    rim_color: Wire,
    rim_strength: Wire,
    rim_width: Wire,
}

fn settings(b: &mut Builder<'_>) -> Settings {
    Settings {
        ramp: b.parameter(map("ramp_texture", TextureKind::Ramp, RAMP, "Shadow ramp")),
        day: b.parameter(unit("day", 1.0, RAMP, "Day (0 night rows, 1 day rows)")),
        night_shade: b.parameter(color(
            "night_shade_color",
            [0.62, 0.62, 0.8],
            SHADE,
            "Night shade colour",
        )),
        ambient: ambient_strength(b),
        vertex_controls: vertex_controls(b),
        rim_color: b.parameter(color("rim_color", [1.0, 1.0, 1.0], RIM, "Rim colour")),
        rim_strength: b.parameter(float("rim_strength", 0.15, (0.0, 1.0), RIM, "Rim strength")),
        rim_width: b.parameter(float("rim_width", 1.0, (0.0, 4.0), RIM, "Rim width")),
    }
}

fn groups(b: &mut Builder<'_>) {
    common(b);
    let scalar = |value: f32| Some(Constant::scalar(value));
    let white = Some(Constant::vec3(1.0, 1.0, 1.0));
    group(
        b,
        LIGHTMAP,
        "Splits the lightmap (ILM): spec = R (specular intensity, above 0.9 metal), bias = G (shadow threshold bias, 0.5 neutral, above 0.95 always lit, below 0.05 always shadow), threshold = B (painted highlight shape), id = A binned into material IDs 0…4 (ramp rows). Without a lightmap the texel is (0, 0.5, 0, 0): no specular, no bias, ID 0. With vertex_controls on, occlusion = vertex colour R and ramp_width = max(0.01, 2 × vertex colour G); else both are 1.",
        &[
            ("texture", ValueType::Texture, None),
            ("uv", ValueType::Float2, None),
            ("vertex_controls", ValueType::Float, scalar(0.0)),
        ],
        &[
            ("spec", ValueType::Float),
            ("bias", ValueType::Float),
            ("threshold", ValueType::Float),
            ("id", ValueType::Float),
            ("occlusion", ValueType::Float),
            ("ramp_width", ValueType::Float),
        ],
        |b| {
            let texture = b.function_input("texture");
            let uv = b.function_input("uv");
            let controls = b.function_input("vertex_controls");
            let (texel, _) = optional_map(b, &texture, [0.0, 0.5, 0.0, 0.0], &uv);
            let spec = b.swizzle(texel.clone(), "r");
            let bias = b.swizzle(texel.clone(), "g");
            let threshold = b.swizzle(texel.clone(), "b");
            let alpha = b.swizzle(texel, "a");
            let id = node(
                b,
                "npr.material_id",
                [("value", alpha.into()), ("mode", 0.0.into())],
            );
            let id = b.out(id, "index");
            let vertex = b.input(Input::VertexColor);
            let red = b.swizzle(vertex.clone(), "r");
            let green = b.swizzle(vertex, "g");
            let occlusion = b.lerp(1.0, red, controls.clone());
            let widened = b.multiply(green, 2.0);
            let widened = maximum(b, widened, 0.01);
            let ramp_width = b.lerp(1.0, widened, controls);
            b.function_output("spec", spec);
            b.function_output("bias", bias);
            b.function_output("threshold", threshold);
            b.function_output("id", id);
            b.function_output("occlusion", occlusion);
            b.function_output("ramp_width", ramp_width);
        },
    );
    group(
        b,
        RAMP_COLOR,
        "The shadow colour at ramp position u for material row id: the ramp atlas row id (day half) blended towards row id + 5 (night half) as day falls; without a ramp texture, shade (day) or night_shade (night), lightening a third of the way to white over the last tenth of u towards the terminator.",
        &[
            ("ramp", ValueType::Texture, None),
            ("u", ValueType::Float, scalar(0.0)),
            ("id", ValueType::Float, scalar(0.0)),
            ("day", ValueType::Float, scalar(1.0)),
            ("shade", ValueType::Float3, white),
            ("night_shade", ValueType::Float3, white),
        ],
        &[("color", ValueType::Float3)],
        |b| {
            let ramp = b.function_input("ramp");
            let u = b.function_input("u");
            let id = b.function_input("id");
            let day = b.function_input("day");
            let shade = b.function_input("shade");
            let night_shade = b.function_input("night_shade");
            let night = one_minus(b, day);
            let sampled = node(
                b,
                "npr.ramp",
                [
                    ("texture", ramp.clone().into()),
                    ("u", u.clone().into()),
                    ("row", id.into()),
                    ("rows", RAMP_ROWS.into()),
                    ("second_offset", NIGHT_ROWS.into()),
                    ("blend", night.clone().into()),
                ],
            );
            let sampled = b.out(sampled, "rgb");
            let bound = node(b, "texture.bound", [("texture", ramp.into())]);
            let bound = b.out(bound, "value");
            let edge = edge_ramp(b, u, [0.9, 1.0]);
            let day_edge = b.lerp(shade.clone(), [1.0, 1.0, 1.0], 0.35);
            let day_color = b.lerp(shade, day_edge, edge.clone());
            let night_edge = b.lerp(night_shade.clone(), [1.0, 1.0, 1.0], 0.35);
            let night_color = b.lerp(night_shade, night_edge, edge);
            let fallback = b.lerp(day_color, night_color, night);
            let color = b.lerp(fallback, sampled, bound);
            b.function_output("color", color);
        },
    );
    group(
        b,
        RAMP_SHADE,
        "Genshin diffuse for the light being added: area = (HL(N·L) + (2x|x| + 0.5) × occlusion) / 2 with x = bias - 0.5 (forced to 1 above 0.95 and 0 below 0.05), in_shadow below light_area; u = 1 - (light_area - area) / (light_area × ramp_width) runs from 1 at the terminator to 0 deep in shadow, times the cast shadow (ignored where bias > 0.8); the ramp colour multiplies albedo only on the shadow side and in cast shadow (lit = 0 there).",
        &[
            ("normal", ValueType::Float3, None),
            ("albedo", ValueType::Float3, white),
            ("bias", ValueType::Float, scalar(0.5)),
            ("occlusion", ValueType::Float, scalar(1.0)),
            ("id", ValueType::Float, scalar(0.0)),
            ("ramp_width", ValueType::Float, scalar(1.0)),
            ("light_area", ValueType::Float, scalar(LIGHT_AREA)),
            ("ramp", ValueType::Texture, None),
            ("day", ValueType::Float, scalar(1.0)),
            ("shade", ValueType::Float3, white),
            ("night_shade", ValueType::Float3, white),
        ],
        &[
            ("color", ValueType::Float3),
            ("in_shadow", ValueType::Float),
            ("lit", ValueType::Float),
        ],
        |b| {
            let normal = b.function_input("normal");
            let albedo = b.function_input("albedo");
            let bias = b.function_input("bias");
            let occlusion = b.function_input("occlusion");
            let id = b.function_input("id");
            let ramp_width = b.function_input("ramp_width");
            let light_area = b.function_input("light_area");
            let ramp = b.function_input("ramp");
            let day = b.function_input("day");
            let shade = b.function_input("shade");
            let night_shade = b.function_input("night_shade");
            let facing = node(
                b,
                "npr.wrap",
                [("normal", normal.into()), ("wrap", 0.0.into())],
            );
            let facing = b.out(facing, "value");
            let area = node(
                b,
                "npr.threshold",
                [
                    ("facing", facing.into()),
                    ("bias", bias.clone().into()),
                    ("mode", 0.0.into()),
                    ("occlusion", occlusion.into()),
                ],
            );
            let area = b.out(area, "value");
            let in_shadow = compare(b, area.clone(), light_area.clone(), Compare::Less);
            let feather = b.multiply(light_area.clone(), ramp_width);
            let u = node(
                b,
                "npr.toon_step",
                [
                    ("value", area.into()),
                    ("edge", light_area.into()),
                    ("feather", feather.into()),
                    ("mode", 3.0.into()),
                    ("softness", 1.0.into()),
                ],
            );
            let u = b.out(u, "value");
            let cast = node(
                b,
                "npr.cast_gate",
                [
                    ("lit", 1.0.into()),
                    ("floor", 0.0.into()),
                    ("mask", bias.into()),
                    ("bypass", 0.8.into()),
                ],
            );
            let cast = b.out(cast, "value");
            let position = b.multiply(u, cast.clone());
            let ramp_color = call(
                b,
                RAMP_COLOR,
                [
                    ("ramp", ramp.into()),
                    ("u", position.into()),
                    ("id", id.into()),
                    ("day", day.into()),
                    ("shade", shade.into()),
                    ("night_shade", night_shade.into()),
                ],
            );
            let ramp_color = b.out(ramp_color, "color");
            let uncast = one_minus(b, cast);
            let shadowed = b.add(in_shadow.clone(), uncast);
            let shadowed = b.saturate(shadowed);
            let lit = one_minus(b, shadowed);
            let color = node(
                b,
                "npr.shade_mix",
                [
                    ("albedo", albedo.into()),
                    ("lit", lit.clone().into()),
                    ("shade", ramp_color.into()),
                    ("mode", 0.0.into()),
                ],
            );
            let color = b.out(color, "value");
            b.function_output("color", color);
            b.function_output("in_shadow", in_shadow);
            b.function_output("lit", lit);
        },
    );
    group(
        b,
        DEPTH_RIM,
        "Genshin's horizontal depth rim: mask = where the scene depth a few pixels sideways (width, shrinking with distance, scaled by the head) lies 0.015…0.1 m behind; rim = mask × (front + back halves against normalize(V - L), power 7.5) × color × strength × albedo × 5 × saturate(light + 0.1).",
        &[
            ("normal", ValueType::Float3, None),
            ("albedo", ValueType::Float3, white),
            ("light", ValueType::Float3, white),
            ("color", ValueType::Float3, white),
            ("strength", ValueType::Float, scalar(0.15)),
            ("width", ValueType::Float, scalar(1.0)),
        ],
        &[("rim", ValueType::Float3)],
        |b| {
            let normal = b.function_input("normal");
            let albedo = b.function_input("albedo");
            let light = b.function_input("light");
            let color = b.function_input("color");
            let strength = b.function_input("strength");
            let width = b.function_input("width");
            let head = node(b, "npr.head", []);
            let scale = b.out(head, "scale");
            let pixels = b.multiply(width, 3.0);
            let mask = node(
                b,
                "npr.depth_rim",
                [
                    ("normal", normal.clone().into()),
                    ("width", pixels.into()),
                    ("direction", 0.0.into()),
                    ("scaling", 1.0.into()),
                    ("low", 0.015.into()),
                    ("high", 0.1.into()),
                    ("scale", scale.into()),
                ],
            );
            let mask = b.out(mask, "mask");
            let halves = node(
                b,
                "npr.split_rim",
                [("normal", normal.into()), ("power", 7.5.into())],
            );
            let front = b.out(halves, "front");
            let back = b.out(halves, "back");
            let front = b.multiply(front, 1.0);
            let back = b.multiply(back, 1.0);
            let shape = b.add(front, back);
            let shape = b.multiply(shape, mask);
            let tinted = b.multiply(color, strength);
            let tinted = b.multiply(tinted, albedo);
            let tinted = b.multiply(tinted, 5.0);
            let lifted = b.add(light, 0.1);
            let lifted = b.saturate(lifted);
            let rim = b.multiply(tinted, lifted);
            let rim = b.multiply(rim, shape);
            b.function_output("rim", rim);
        },
    );
}

fn specular_groups(b: &mut Builder<'_>) {
    let scalar = |value: f32| Some(Constant::scalar(value));
    let white = Some(Constant::vec3(1.0, 1.0, 1.0));
    group(
        b,
        SPECULAR_GROUP,
        "Genshin's stepped Blinn-Phong: the highlight shows where max(N·H, 0.001)^shininess exceeds 1.015 - threshold (lightmap B), as term × spec (lightmap R) × strength × 0.5 × color, tinted towards albedo by tint; metal = 1 where spec exceeds 0.89, which zeroes this highlight.",
        &[
            ("normal", ValueType::Float3, None),
            ("albedo", ValueType::Float3, white),
            ("spec", ValueType::Float, scalar(0.0)),
            ("threshold", ValueType::Float, scalar(0.0)),
            ("color", ValueType::Float3, white),
            ("shininess", ValueType::Float, scalar(10.0)),
            ("strength", ValueType::Float, scalar(1.0)),
            ("tint", ValueType::Float, scalar(0.5)),
        ],
        &[("color", ValueType::Float3), ("metal", ValueType::Float)],
        |b| {
            let normal = b.function_input("normal");
            let albedo = b.function_input("albedo");
            let spec = b.function_input("spec");
            let threshold = b.function_input("threshold");
            let color = b.function_input("color");
            let shininess = b.function_input("shininess");
            let strength = b.function_input("strength");
            let tint = b.function_input("tint");
            let stepped = node(
                b,
                "npr.stepped_specular",
                [
                    ("normal", normal.into()),
                    ("shininess", shininess.into()),
                    ("threshold", threshold.into()),
                    ("intensity", 1.0.into()),
                    ("softness", 0.0.into()),
                    ("offset", 1.015.into()),
                ],
            );
            let term = b.out(stepped, "term");
            let revealed = b.out(stepped, "revealed");
            let metal = compare(b, spec.clone(), 0.89, Compare::Greater);
            let amount = b.multiply(term, revealed);
            let amount = b.multiply(amount, spec);
            let amount = b.multiply(amount, strength);
            let amount = b.multiply(amount, 0.5);
            let plain = one_minus(b, metal.clone());
            let amount = b.multiply(amount, plain);
            let tinted = b.lerp([1.0, 1.0, 1.0], albedo, tint);
            let tinted = b.multiply(tinted, color);
            let highlight = b.multiply(tinted, amount);
            b.function_output("color", highlight);
            b.function_output("metal", metal);
        },
    );
    group(
        b,
        METAL,
        "Genshin metal: the grey metal matcap (view-space normal, red channel) × brightness picks between dark and light, times albedo, plus half the highlight saturate(max(N·H, 0.001)^shininess × scale) × spec; in shadow the highlight drops to a fifth and the colour takes a cool tint.",
        &[
            ("normal", ValueType::Float3, None),
            ("albedo", ValueType::Float3, white),
            ("matcap", ValueType::Texture, None),
            ("spec", ValueType::Float, scalar(1.0)),
            ("in_shadow", ValueType::Float, scalar(0.0)),
            ("dark", ValueType::Float3, white),
            ("light", ValueType::Float3, white),
            ("brightness", ValueType::Float, scalar(3.0)),
            ("shininess", ValueType::Float, scalar(90.0)),
            ("scale", ValueType::Float, scalar(15.0)),
        ],
        &[("color", ValueType::Float3)],
        |b| {
            let normal = b.function_input("normal");
            let albedo = b.function_input("albedo");
            let matcap = b.function_input("matcap");
            let spec = b.function_input("spec");
            let in_shadow = b.function_input("in_shadow");
            let dark = b.function_input("dark");
            let light = b.function_input("light");
            let brightness = b.function_input("brightness");
            let shininess = b.function_input("shininess");
            let scale = b.function_input("scale");
            let uv = node(
                b,
                "npr.matcap_uv",
                [
                    ("normal", normal.clone().into()),
                    ("mode", 1.0.into()),
                    ("flip", 1.0.into()),
                ],
            );
            let uv = b.out(uv, "uv");
            let sampled = b.texture(
                TextureKind::Colour,
                [("texture", matcap.into()), ("uv", uv.into())],
            );
            let value = b.out(sampled, "rgba");
            let value = b.swizzle(value, "r");
            let lobe = node(
                b,
                "npr.stepped_specular",
                [
                    ("normal", normal.into()),
                    ("shininess", shininess.into()),
                    ("threshold", 1.0.into()),
                ],
            );
            let term = b.out(lobe, "term");
            let term = b.multiply(term, scale);
            let term = b.saturate(term);
            let metal = node(
                b,
                "npr.metal",
                [
                    ("albedo", albedo.into()),
                    ("value", value.into()),
                    ("dark", dark.into()),
                    ("light", light.into()),
                    ("brightness", brightness.into()),
                    ("term", term.into()),
                    ("sharp_edge", 1.0.into()),
                    ("intensity", spec.into()),
                    ("shadow", in_shadow.into()),
                    ("shadow_specular", 0.2.into()),
                    ("shadow_color", [0.78, 0.77, 0.82].into()),
                ],
            );
            let metal = b.out(metal, "value");
            b.function_output("color", metal);
        },
    );
}

struct Highlights {
    color: Wire,
    shininess: Wire,
    strength: Wire,
    tint: Wire,
}

fn highlights(b: &mut Builder<'_>) -> Highlights {
    Highlights {
        color: b.parameter(color(
            "specular_color",
            [1.0, 1.0, 1.0],
            SPECULAR,
            "Specular colour",
        )),
        shininess: b.parameter(float(
            "specular_shininess",
            10.0,
            (1.0, 128.0),
            SPECULAR,
            "Specular shininess",
        )),
        strength: b.parameter(float(
            "specular_strength",
            1.0,
            (0.0, 4.0),
            SPECULAR,
            "Specular strength",
        )),
        tint: b.parameter(unit(
            "specular_tint",
            0.5,
            SPECULAR,
            "Tint highlight by base",
        )),
    }
}

struct Metal {
    matcap: Wire,
    dark: Wire,
    light: Wire,
    brightness: Wire,
    shininess: Wire,
    scale: Wire,
}

fn metal(b: &mut Builder<'_>) -> Metal {
    Metal {
        matcap: b.parameter(map(
            "metal_matcap",
            TextureKind::Colour,
            METAL_GROUP,
            "Metal matcap",
        )),
        dark: b.parameter(color(
            "metal_dark_color",
            [0.51, 0.36, 0.37],
            METAL_GROUP,
            "Metal dark colour",
        )),
        light: b.parameter(color(
            "metal_light_color",
            [1.0, 1.0, 1.0],
            METAL_GROUP,
            "Metal light colour",
        )),
        brightness: b.parameter(float(
            "metal_brightness",
            3.0,
            (0.0, 8.0),
            METAL_GROUP,
            "Metal brightness",
        )),
        shininess: b.parameter(float(
            "metal_shininess",
            90.0,
            (1.0, 256.0),
            METAL_GROUP,
            "Metal shininess",
        )),
        scale: b.parameter(float(
            "metal_specular_scale",
            15.0,
            (0.0, 64.0),
            METAL_GROUP,
            "Metal highlight scale",
        )),
    }
}

fn body(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    groups(&mut builder);
    specular_groups(&mut builder);
    let b = &mut builder;
    let imports = imports(b, &DEFAULTS);
    let settings = settings(b);
    let use_alpha = alpha_use(b, "emission");

    let (surface, lightmap) = b.frame("Surface and lightmap", |b| {
        let surface = base(b, &imports);
        let texture = b.parameter(map(
            "lightmap_texture",
            TextureKind::Data,
            LIGHTMAP_GROUP,
            "Lightmap (R specular, G shadow bias, B highlight, A material ID)",
        ));
        let split = call(
            b,
            LIGHTMAP,
            [
                ("texture", texture.into()),
                ("uv", surface.uv.clone().into()),
                ("vertex_controls", settings.vertex_controls.clone().into()),
            ],
        );
        (surface, split)
    });
    let spec = b.out(lightmap, "spec");
    let bias = b.out(lightmap, "bias");
    let threshold = b.out(lightmap, "threshold");
    let id = b.out(lightmap, "id");
    let occlusion = b.out(lightmap, "occlusion");
    let vertex_width = b.out(lightmap, "ramp_width");

    let (diffuse, in_shadow, lit) = b.frame("Ramp shading", |b| {
        let width = b.parameter(float(
            "ramp_width",
            1.0,
            (0.05, 2.0),
            SHADE,
            "Shadow ramp width",
        ));
        let width = b.multiply(width, vertex_width);
        let shaded = call(
            b,
            RAMP_SHADE,
            [
                ("normal", surface.normal.clone().into()),
                ("albedo", surface.albedo.clone().into()),
                ("bias", bias.clone().into()),
                ("occlusion", occlusion.into()),
                ("id", id.clone().into()),
                ("ramp_width", width.into()),
                ("ramp", settings.ramp.clone().into()),
                ("day", settings.day.clone().into()),
                ("shade", imports.shade_color.clone().into()),
                ("night_shade", settings.night_shade.clone().into()),
            ],
        );
        (
            b.out(shaded, "color"),
            b.out(shaded, "in_shadow"),
            b.out(shaded, "lit"),
        )
    });

    let material = b.frame("Specular and metal", |b| {
        let highlights = highlights(b);
        let metal = metal(b);
        let highlight = call(
            b,
            SPECULAR_GROUP,
            [
                ("normal", surface.normal.clone().into()),
                ("albedo", surface.albedo.clone().into()),
                ("spec", spec.clone().into()),
                ("threshold", threshold.into()),
                ("color", highlights.color.into()),
                ("shininess", highlights.shininess.into()),
                ("strength", highlights.strength.into()),
                ("tint", highlights.tint.into()),
            ],
        );
        let shine = call(
            b,
            METAL,
            [
                ("normal", surface.normal.clone().into()),
                ("albedo", surface.albedo.clone().into()),
                ("matcap", metal.matcap.into()),
                ("spec", spec.clone().into()),
                ("in_shadow", in_shadow.into()),
                ("dark", metal.dark.into()),
                ("light", metal.light.into()),
                ("brightness", metal.brightness.into()),
                ("shininess", metal.shininess.into()),
                ("scale", metal.scale.into()),
            ],
        );
        let highlight_color = b.out(highlight, "color");
        let is_metal = b.out(highlight, "metal");
        let plain = b.add(diffuse, highlight_color);
        let shine = b.out(shine, "color");
        b.lerp(plain, shine, is_metal)
    });

    let light = b.frame("Lights", |b| {
        let bright = compare(b, bias.clone(), 0.9, Compare::Greater);
        let open = one_minus(b, bright);
        let extra = b.multiply(surface.albedo.clone(), lit);
        let extra = b.multiply(extra, open);
        lights(
            b,
            &settings,
            &surface.normal,
            &surface.albedo,
            material,
            extra,
        )
    });

    let (surface_shader, alpha) = b.frame("Ambient and emission", |b| {
        let shader = finish_surface(
            b,
            &imports,
            &settings,
            &surface.albedo,
            &surface.alpha,
            &use_alpha,
            light,
        );
        let alpha = b.lerp(surface.alpha.clone(), 1.0, use_alpha.clone());
        (shader, alpha)
    });

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
                ("index", id.clone().into()),
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
        let one = b.constant(Constant::scalar(1.0));
        outline(b, &imports, &settings, &picked, &one)
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

fn lights(
    b: &mut Builder<'_>,
    settings: &Settings,
    normal: &Wire,
    albedo: &Wire,
    key: Wire,
    extra: Wire,
) -> Wire {
    let incoming = incoming(b);
    let rim = call(
        b,
        DEPTH_RIM,
        [
            ("normal", normal.into()),
            ("albedo", albedo.into()),
            ("light", incoming.color.clone().into()),
            ("color", settings.rim_color.clone().into()),
            ("strength", settings.rim_strength.clone().into()),
            ("width", settings.rim_width.clone().into()),
        ],
    );
    let rim = b.out(rim, "rim");
    let key = b.multiply(key, incoming.color.clone());
    let extra = b.multiply(extra, 0.5);
    let extra = b.multiply(extra, incoming.color);
    let shaded = b.lerp(extra, key, incoming.leading.clone());
    let rim = b.multiply(rim, incoming.leading);
    b.add(shaded, rim)
}

fn finish_surface(
    b: &mut Builder<'_>,
    imports: &Imports,
    settings: &Settings,
    albedo: &Wire,
    alpha: &Wire,
    use_alpha: &Wire,
    light: Wire,
) -> Wire {
    let ambient = flat_ambient(b, albedo, &settings.ambient);
    let lighting = b.shader(
        NodeKind::Lighting,
        [("light", light.into()), ("ambient", ambient.into())],
    );
    let texture = b.texture(
        TextureKind::Colour,
        [("texture", imports.emission_texture.clone().into())],
    );
    let texture = b.out(texture, "rgb");
    let masked = b.multiply(albedo, alpha);
    let source = b.lerp(texture, masked, use_alpha);
    let color = b.multiply(imports.emission_color.clone(), source);
    let emission = b.shader(NodeKind::Emission, [("color", color.into())]);
    b.shader(
        NodeKind::AddShader,
        [("a", lighting.into()), ("b", emission.into())],
    )
}

fn outline(
    b: &mut Builder<'_>,
    imports: &Imports,
    settings: &Settings,
    color: &Wire,
    multiplier: &Wire,
) -> (Wire, OutlineShape) {
    let incoming = incoming(b);
    let lit = b.multiply(color, incoming.color);
    let lit = b.multiply(lit, incoming.leading);
    let ambient = flat_ambient(b, color, &settings.ambient);
    let shader = b.shader(
        NodeKind::Lighting,
        [("light", lit.into()), ("ambient", ambient.into())],
    );
    let shape = outline_shape(
        b,
        &imports.outline_width,
        multiplier,
        &settings.vertex_controls,
        4.0,
        &OutlineStyle {
            mode: 2.0,
            distances: [0.01, 2.0, 6.0],
            scales: [0.43, 1.0, 2.45],
            flatten_z: 0.01,
            depth_mode: 1.0,
            depth_offset: 0.0,
        },
    );
    (shader, shape)
}

fn face(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    groups(&mut builder);
    face_shadow_group(&mut builder);
    let b = &mut builder;
    let imports = imports(b, &DEFAULTS);
    let settings = settings(b);
    let use_alpha = alpha_use(b, "blush");
    let face_map = b.parameter(map(
        "face_map",
        TextureKind::Data,
        FACE,
        "Face map (A shadow SDF, B outline width)",
    ));

    let (surface, albedo) = b.frame("Surface, blush and nose line", |b| {
        let surface = base(b, &imports);
        let blush = b.parameter(color("blush_color", [1.0, 0.62, 0.6], FACE, "Blush colour"));
        let blush_strength = b.parameter(unit("blush_strength", 0.0, FACE, "Blush strength"));
        let blush_mask = b.multiply(surface.alpha.clone(), blush_strength);
        let blush_mask = b.multiply(blush_mask, use_alpha.clone());
        let blushed = b.lerp(surface.albedo.clone(), blush, blush_mask);
        let nose_map = b.parameter(map(
            "nose_line_texture",
            TextureKind::Data,
            FACE,
            "Nose line mask (R)",
        ));
        let (nose_texel, _) = optional_map(b, &nose_map, [0.0, 0.0, 0.0, 0.0], &surface.uv);
        let nose_mask = b.swizzle(nose_texel, "r");
        let nose = node(
            b,
            "npr.nose_line",
            [
                ("mask", nose_mask.into()),
                ("mode", 0.0.into()),
                ("power", 4.0.into()),
                ("threshold", 1.03.into()),
            ],
        );
        let nose = b.out(nose, "value");
        let nose_color = b.parameter(color(
            "nose_line_color",
            [0.66, 0.42, 0.4],
            FACE,
            "Nose line colour",
        ));
        let darkened = b.multiply(blushed.clone(), nose_color);
        let albedo = b.lerp(blushed, darkened, nose);
        (surface, albedo)
    });

    let (face_color, lit) = b.frame("Face shadow", |b| {
        let sample = node(
            b,
            "npr.face_sample",
            [
                ("texture", face_map.clone().into()),
                ("channel", 3.0.into()),
            ],
        );
        let sdf = b.out(sample, "sdf");
        let mirrored = b.out(sample, "mirrored");
        let bound = node(b, "texture.bound", [("texture", face_map.clone().into())]);
        let bound = b.out(bound, "value");
        let mirror = b.parameter(choice(
            "face_mirror",
            &["left", "right"],
            0,
            FACE,
            "Mirror the SDF when the light is on the face's",
        ));
        let softness = b.parameter(float(
            "face_shadow_softness",
            0.02,
            (0.0, 0.2),
            FACE,
            "Face shadow softness",
        ));
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
        let lit = node(
            b,
            "npr.cast_gate",
            [("lit", lit.into()), ("floor", 0.0.into())],
        );
        let lit = b.out(lit, "value");
        let row = b.parameter(integer("face_ramp_row", 0, RAMP, "Skin row of the ramp"));
        let ramp_color = call(
            b,
            RAMP_COLOR,
            [
                ("ramp", settings.ramp.clone().into()),
                ("u", lit.clone().into()),
                ("id", row.into()),
                ("day", settings.day.clone().into()),
                ("shade", imports.shade_color.clone().into()),
                ("night_shade", settings.night_shade.clone().into()),
            ],
        );
        let ramp_color = b.out(ramp_color, "color");
        let color = node(
            b,
            "npr.shade_mix",
            [
                ("albedo", albedo.clone().into()),
                ("lit", lit.clone().into()),
                ("shade", ramp_color.into()),
                ("mode", 0.0.into()),
            ],
        );
        (b.out(color, "value"), lit)
    });

    let light = b.frame("Lights", |b| {
        let extra = b.multiply(albedo.clone(), lit);
        lights(b, &settings, &surface.normal, &albedo, face_color, extra)
    });

    let (surface_shader, alpha) = b.frame("Ambient and emission", |b| {
        let zero = b.constant(Constant::scalar(0.0));
        let shader = finish_surface(
            b,
            &imports,
            &settings,
            &albedo,
            &surface.alpha,
            &zero,
            light,
        );
        let alpha = b.lerp(surface.alpha.clone(), 1.0, use_alpha.clone());
        (shader, alpha)
    });

    let (outline_shader, shape) = b.frame("Outline", |b| {
        let (texel, _) = optional_map(b, &face_map, [1.0, 1.0, 1.0, 1.0], &surface.uv);
        let multiplier = b.swizzle(texel, "b");
        outline(b, &imports, &settings, &imports.outline_color, &multiplier)
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
