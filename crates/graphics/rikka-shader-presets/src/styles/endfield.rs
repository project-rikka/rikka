use rikka_shader_graph::{Builder, Error, Graph, GraphKind, Input, Library, Op, Wire};
use rikka_slang::TextureKind;

use super::blocks::{self, Gloss};
use super::kit;
use crate::{Family, Preset};

pub(super) fn presets() -> Vec<Preset> {
    let game = |key, title, summary, build| Preset {
        key,
        title,
        summary,
        family: Family::Game,
        build,
    };
    vec![
        game(
            "endfield_body",
            "Endfield body",
            "Arknights: Endfield-style anime realism for clothes and gear: physically based specular, metals and fabric sheen under real lights over a soft two-band cel diffuse with a cool shade, light-side depth rim and optional brush strokes.",
            endfield_body,
        ),
        game(
            "endfield_skin",
            "Endfield skin",
            "Endfield-style skin: a soft cel diffuse with a warm terminator and rim shadow, blended with screen-space subsurface scattering, under a faint physically based dual-lobe sheen.",
            endfield_skin,
        ),
        game(
            "endfield_hair",
            "Endfield hair",
            "Endfield-style hair: cel diffuse, the physically based primary strand highlight and a stylized tinted band that slides along the strands with the light.",
            endfield_hair,
        ),
        game(
            "endfield_face",
            "Endfield face",
            "Endfield-style face: an SDF face shadow (procedural without a map) with a warm terminator, screen-space hair shadow, light-side rim and faint physically based specular.",
            endfield_face,
        ),
    ]
}

fn defined(library: &Library) -> Builder<'_> {
    let mut builder = Builder::new(GraphKind::Material, library);
    blocks::define_cel_band(&mut builder);
    blocks::define_brush(&mut builder);
    blocks::define_lit(&mut builder);
    blocks::define_light_rim(&mut builder);
    blocks::define_ambient(&mut builder);
    builder
}

struct Mask {
    metalness: Wire,
    roughness: Wire,
    occlusion: Wire,
    rim: Wire,
}

fn mask(builder: &mut Builder<'_>, metalness: f32, roughness: f32) -> Mask {
    builder.frame("Material mask", |b| {
        let texture = b.parameter(
            kit::texture("mask_texture", TextureKind::Data)
                .with_group("material")
                .with_label("Mask (metal, roughness, occlusion, rim)"),
        );
        let metal = b.parameter(
            kit::unit("base_metalness", metalness)
                .with_group("material")
                .with_label("Metalness")
                .with_role(rikka_slang::Role::Metalness),
        );
        let rough = b.parameter(
            kit::unit("specular_roughness", roughness)
                .with_group("material")
                .with_label("Roughness"),
        );
        let strength = b.parameter(
            kit::unit("occlusion_strength", 1.0)
                .with_group("material")
                .with_label("Occlusion strength"),
        );
        let texel = kit::data(b, &texture);
        let red = kit::channel(b, &texel, "x");
        let green = kit::channel(b, &texel, "y");
        let blue = kit::channel(b, &texel, "z");
        let alpha = kit::channel(b, &texel, "w");
        Mask {
            metalness: b.multiply(metal, red),
            roughness: b.multiply(rough, green),
            occlusion: b.lerp(1.0, blue, strength),
            rim: alpha,
        }
    })
}

struct Look {
    shade: [f32; 3],
    edge: f32,
    softness: f32,
    terminator: [f32; 3],
    terminator_strength: f32,
    flatten: f32,
}

struct Cel {
    shade: Wire,
    edge: Wire,
    softness: Wire,
    terminator: Wire,
    width: Wire,
    strength: Wire,
    flatten: Wire,
}

fn cel(builder: &mut Builder<'_>, look: &Look) -> Cel {
    builder.frame("Cel look", |b| {
        let group = "cel";
        Cel {
            shade: b.parameter(super::shade_color(look.shade)),
            edge: b.parameter(
                kit::unit("band_edge", look.edge)
                    .with_group(group)
                    .with_label("Shadow edge"),
            ),
            softness: b.parameter(
                kit::amount("band_softness", look.softness, 0.0, 0.5)
                    .with_group(group)
                    .with_label("Edge softness"),
            ),
            terminator: b.parameter(
                kit::color("terminator_color", look.terminator)
                    .with_group(group)
                    .with_label("Terminator colour"),
            ),
            width: b.parameter(
                kit::amount("terminator_width", 0.12, 0.0, 0.5)
                    .with_group(group)
                    .with_label("Terminator width"),
            ),
            strength: b.parameter(
                kit::unit("terminator_strength", look.terminator_strength)
                    .with_group(group)
                    .with_label("Terminator strength"),
            ),
            flatten: b.parameter(
                kit::unit("ambient_flatten", look.flatten)
                    .with_group(group)
                    .with_label("Flat ambient"),
            ),
        }
    })
}

struct Strokes {
    amount: Wire,
    scale: Wire,
    stretch: Wire,
}

fn strokes(builder: &mut Builder<'_>) -> Strokes {
    builder.frame("Brush strokes", |b| Strokes {
        amount: b.parameter(
            kit::amount("brush_amount", 0.0, 0.0, 0.5)
                .with_group("brush")
                .with_label("Amount"),
        ),
        scale: b.parameter(
            kit::amount("brush_scale", 8.0, 1.0, 64.0)
                .with_group("brush")
                .with_label("Scale"),
        ),
        stretch: b.parameter(
            kit::amount("brush_stretch", 4.0, 1.0, 16.0)
                .with_group("brush")
                .with_label("Stretch"),
        ),
    })
}

fn response(builder: &mut Builder<'_>, facing: Wire, occlusion: &Wire, strokes: &Strokes) -> Wire {
    let shadow = kit::value(builder, "light.shadow", "visibility", []);
    let value = builder.multiply(facing, occlusion);
    let value = builder.multiply(value, shadow);
    let at = kit::uv(builder);
    let toward = builder.input(Input::LightDirection);
    let jitter = blocks::brush(
        builder,
        [
            ("uv", at.into()),
            ("light", toward.into()),
            ("amount", (&strokes.amount).into()),
            ("scale", (&strokes.scale).into()),
            ("stretch", (&strokes.stretch).into()),
        ],
    );
    builder.add(value, jitter)
}

fn banded(builder: &mut Builder<'_>, value: Wire, cel: &Cel) -> Wire {
    blocks::cel_tone(
        builder,
        [
            ("value", value.into()),
            ("edge", (&cel.edge).into()),
            ("softness", (&cel.softness).into()),
            ("shade", (&cel.shade).into()),
            ("terminator", (&cel.terminator).into()),
            ("terminator_width", (&cel.width).into()),
            ("terminator_strength", (&cel.strength).into()),
        ],
    )
}

fn rim(builder: &mut Builder<'_>, normal: &Wire, mask: Option<&Wire>) -> Wire {
    builder.frame("Light-side depth rim", |b| {
        let width = b.parameter(
            kit::amount("rim_width", 3.0, 0.0, 12.0)
                .with_group("rim")
                .with_label("Width (pixels)"),
        );
        let color = b.parameter(
            kit::color("rim_color", [1.0, 0.98, 0.95])
                .with_group("rim")
                .with_label("Colour"),
        );
        let strength = b.parameter(
            kit::amount("rim_strength", 0.6, 0.0, 4.0)
                .with_group("rim")
                .with_label("Strength"),
        );
        let found = blocks::depth_rim(b, normal, width);
        let found = match mask {
            Some(mask) => b.multiply(found, mask),
            None => found,
        };
        blocks::light_rim(
            b,
            [
                ("rim", found.into()),
                ("normal", normal.into()),
                ("color", color.into()),
                ("strength", strength.into()),
            ],
        )
    })
}

fn specular_weight(builder: &mut Builder<'_>, value: f32) -> Wire {
    builder.parameter(
        kit::amount("specular_weight", value, 0.0, 1.0)
            .with_group("material")
            .with_label("Specular"),
    )
}

fn finish(
    mut builder: Builder<'_>,
    surface: Wire,
    alpha: Wire,
    albedo: &Wire,
    outline: f32,
) -> Result<Graph, Error> {
    let line = super::outline(&mut builder, albedo, outline, [0.22, 0.2, 0.24]);
    builder.output([
        ("surface", surface.into()),
        ("alpha", alpha.into()),
        ("outline", line.shader.into()),
        ("outline_width", line.width.into()),
    ]);
    builder.finish()
}

fn endfield_body(library: &Library) -> Result<Graph, Error> {
    let mut b = defined(library);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let mask = mask(&mut b, 0.0, 0.5);
    let cel = cel(
        &mut b,
        &Look {
            shade: [0.6, 0.63, 0.74],
            edge: 0.5,
            softness: 0.12,
            terminator: [0.82, 0.72, 0.7],
            terminator_strength: 0.15,
            flatten: 0.3,
        },
    );
    let strokes = strokes(&mut b);
    let diffuse_albedo = b.frame("Diffuse albedo", |b| {
        let dielectric = kit::one_minus(b, &mask.metalness);
        b.multiply(&albedo.rgb, dielectric)
    });
    let diffuse = b.frame("Cel diffuse (per light)", |b| {
        let facing = kit::value(b, "npr.wrap", "value", [("normal", (&normal).into())]);
        let value = response(b, facing, &mask.occlusion, &strokes);
        let tone = banded(b, value, &cel);
        blocks::lit(b, &diffuse_albedo, tone)
    });
    let rim = rim(&mut b, &normal, Some(&mask.rim));
    let flat = cel.flatten.clone();
    let ambient = b.frame("Ambient", |b| {
        blocks::ambient(
            b,
            [
                ("albedo", (&diffuse_albedo).into()),
                ("normal", (&normal).into()),
                ("occlusion", (&mask.occlusion).into()),
                ("flatten", flat.into()),
            ],
        )
    });
    let light = b.add(diffuse, rim);
    let cel_shader = kit::lighting(&mut b, &light, &ambient);
    let weight = specular_weight(&mut b, 1.0);
    let gloss = blocks::specular_layer(
        &mut b,
        Gloss {
            albedo: &albedo.rgb,
            metalness: (&mask.metalness).into(),
            roughness: (&mask.roughness).into(),
            weight: weight.into(),
            normal: (&normal).into(),
            occlusion: (&mask.occlusion).into(),
        },
    );
    b.frame("Fabric sheen", |b| {
        let weight = b.parameter(
            kit::unit("fuzz_weight", 0.0)
                .with_group("material")
                .with_label("Sheen"),
        );
        let color = b.parameter(
            kit::color("fuzz_color", [1.0, 1.0, 1.0])
                .with_group("material")
                .with_label("Sheen colour"),
        );
        b.set(gloss.node(), "fuzz_weight", weight)
            .set(gloss.node(), "fuzz_color", color);
    });
    let glow = super::emitted(&mut b);
    let glow = kit::emission(&mut b, glow);
    let surface = kit::add_shader(&mut b, &cel_shader, &gloss);
    let surface = kit::add_shader(&mut b, &surface, &glow);
    finish(b, surface, albedo.alpha, &albedo.rgb, 0.0008)
}

fn endfield_skin(library: &Library) -> Result<Graph, Error> {
    let mut b = defined(library);
    let albedo = super::albedo(&mut b, [1.0, 0.86, 0.8, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let mask = mask(&mut b, 0.0, 0.55);
    let cel = cel(
        &mut b,
        &Look {
            shade: [0.86, 0.66, 0.66],
            edge: 0.45,
            softness: 0.16,
            terminator: [0.98, 0.5, 0.4],
            terminator_strength: 0.35,
            flatten: 0.4,
        },
    );
    let strokes = strokes(&mut b);
    let shaded = b.frame("Rim shadow", |b| {
        let color = b.parameter(
            kit::color("rim_shadow_color", [0.62, 0.42, 0.42])
                .with_group("cel")
                .with_label("Rim shadow colour"),
        );
        let strength = b.parameter(
            kit::amount("rim_shadow_strength", 1.0, 0.0, 4.0)
                .with_group("cel")
                .with_label("Rim shadow"),
        );
        let darkening = kit::value(
            b,
            "npr.rim_shadow",
            "value",
            [
                ("normal", (&normal).into()),
                ("color", color.into()),
                ("intensity", strength.into()),
            ],
        );
        b.multiply(&albedo.rgb, darkening)
    });
    let share = b.parameter(
        kit::unit("subsurface_mix", 0.3)
            .with_group("subsurface")
            .with_label("Scattered share"),
    );
    let cel_albedo = b.frame("Cel share", |b| {
        let rest = kit::one_minus(b, &share);
        b.multiply(&shaded, rest)
    });
    let diffuse = b.frame("Cel diffuse (per light)", |b| {
        let facing = kit::value(b, "npr.wrap", "value", [("normal", (&normal).into())]);
        let value = response(b, facing, &mask.occlusion, &strokes);
        let tone = banded(b, value, &cel);
        blocks::lit(b, &cel_albedo, tone)
    });
    let rim = rim(&mut b, &normal, Some(&mask.rim));
    let flat = cel.flatten.clone();
    let ambient = b.frame("Ambient", |b| {
        blocks::ambient(
            b,
            [
                ("albedo", (&cel_albedo).into()),
                ("normal", (&normal).into()),
                ("occlusion", (&mask.occlusion).into()),
                ("flatten", flat.into()),
            ],
        )
    });
    let light = b.add(diffuse, rim);
    let cel_shader = kit::lighting(&mut b, &light, &ambient);
    let weight = specular_weight(&mut b, 0.45);
    let skin = b.frame("Scattered skin and dual-lobe specular", |b| {
        let radius = b.parameter(
            kit::amount("subsurface_radius", 0.004, 0.0, 0.02)
                .with_group("subsurface")
                .with_label("Radius")
                .with_hint(rikka_slang::Hint::Logarithmic),
        );
        let scale = b.parameter(
            kit::vector3("subsurface_radius_scale", [1.0, 0.37, 0.19])
                .with_group("subsurface")
                .with_label("Radius scale"),
        );
        let radius = b.multiply(radius, scale);
        let scattered = b.multiply(&albedo.rgb, &share);
        let skin = kit::node(
            b,
            "bsdf.skin",
            [
                ("color", (&scattered).into()),
                ("subsurface_color", scattered.into()),
                ("subsurface_radius", radius.into()),
                ("roughness", (&mask.roughness).into()),
                ("second_roughness", 0.3.into()),
                ("second_lobe", 0.15.into()),
                ("ior", 1.4.into()),
                ("specular_weight", weight.into()),
                ("normal", (&normal).into()),
                ("occlusion", (&mask.occlusion).into()),
            ],
        );
        b.out(skin, "shader")
    });
    let surface = kit::add_shader(&mut b, &cel_shader, &skin);
    finish(b, surface, albedo.alpha, &albedo.rgb, 0.0006)
}

fn strand_tangent(builder: &mut Builder<'_>, direction: &Wire) -> Wire {
    builder.frame("Strand flow", |b| {
        let texture = b.parameter(
            kit::texture("flow_texture", TextureKind::Data)
                .with_group("strand")
                .with_label("Flow map"),
        );
        let along = b.lerp([1.0, 0.0], [0.0, 1.0], direction);
        let texel = kit::data(b, &texture);
        let painted = kit::channel(b, &texel, "xy");
        let painted = b.multiply(painted, 2.0);
        let painted = b.subtract(painted, 1.0);
        let bound = kit::value(b, "texture.bound", "value", [("texture", texture.into())]);
        let flow = b.lerp(along, painted, bound);
        let x = kit::channel(b, &flow, "x");
        let y = kit::channel(b, &flow, "y");
        let local = kit::combine(b, [x.into(), y.into(), 1e-4.into()]);
        let tangent = kit::value(
            b,
            "vector.from_tangent",
            "value",
            [("vector", local.into())],
        );
        kit::normalize(b, tangent)
    })
}

fn endfield_hair(library: &Library) -> Result<Graph, Error> {
    let mut b = defined(library);
    let albedo = super::albedo(&mut b, [1.0, 1.0, 1.0, 1.0]);
    let normal = super::mapped_normal(&mut b);
    let mask = mask(&mut b, 0.0, 0.35);
    let cel = cel(
        &mut b,
        &Look {
            shade: [0.55, 0.55, 0.68],
            edge: 0.5,
            softness: 0.1,
            terminator: [0.8, 0.7, 0.75],
            terminator_strength: 0.1,
            flatten: 0.3,
        },
    );
    let strokes = strokes(&mut b);
    let direction = b.parameter(
        kit::options("direction", &["u", "v"], 1)
            .with_group("strand")
            .with_label("Strand direction"),
    );
    let tangent = strand_tangent(&mut b, &direction);
    let diffuse = b.frame("Cel diffuse (per light)", |b| {
        let facing = kit::value(b, "npr.wrap", "value", [("normal", (&normal).into())]);
        let value = response(b, facing, &mask.occlusion, &strokes);
        let tone = banded(b, value, &cel);
        blocks::lit(b, &albedo.rgb, tone)
    });
    let jitter = b.frame("Shift map", |b| {
        let shift_texture = b.parameter(
            kit::texture("shift_texture", TextureKind::Data)
                .with_group("strand")
                .with_label("Shift map"),
        );
        let texel = kit::data(b, &shift_texture);
        kit::channel(b, &texel, "x")
    });
    let band = b.frame("Stylized strand band (per light)", |b| {
        let position = b.parameter(
            kit::unit("band_position", 0.35)
                .with_group("strand")
                .with_label("Band position"),
        );
        let width = b.parameter(
            kit::amount("band_width", 0.05, 0.0, 0.3)
                .with_group("strand")
                .with_label("Band width"),
        );
        let slide = b.parameter(
            kit::amount("band_slide", 0.4, -1.0, 1.0)
                .with_group("strand")
                .with_label("Band slide"),
        );
        let tint = b.parameter(
            kit::color("band_color", [1.0, 0.95, 0.9])
                .with_group("strand")
                .with_label("Band colour"),
        );
        let strength = b.parameter(
            kit::amount("band_strength", 0.8, 0.0, 4.0)
                .with_group("strand")
                .with_label("Band strength"),
        );
        let at = kit::uv(b);
        let u = kit::channel(b, &at, "x");
        let v = kit::channel(b, &at, "y");
        let along = b.lerp(u, v, &direction);
        let band = kit::value(
            b,
            "npr.strand_band",
            "value",
            [
                ("gradient", along.into()),
                ("jitter", (&jitter).into()),
                ("tangent", (&tangent).into()),
                ("offset", position.into()),
                ("shift", slide.into()),
                ("width", width.into()),
            ],
        );
        let shadow = kit::value(b, "light.shadow", "visibility", []);
        let band = b.multiply(band, shadow);
        let band = b.multiply(band, strength);
        let colour = b.multiply(&albedo.rgb, tint);
        let colour = b.lerp(colour, [1.0, 1.0, 1.0], 0.5);
        blocks::lit(b, colour, band)
    });
    let rim = rim(&mut b, &normal, Some(&mask.rim));
    let flat = cel.flatten.clone();
    let ambient = b.frame("Ambient", |b| {
        blocks::ambient(
            b,
            [
                ("albedo", (&albedo.rgb).into()),
                ("normal", (&normal).into()),
                ("occlusion", (&mask.occlusion).into()),
                ("flatten", flat.into()),
            ],
        )
    });
    let light = b.add(diffuse, band);
    let light = b.add(light, rim);
    let cel_shader = kit::lighting(&mut b, &light, &ambient);
    let strand = b.frame("Physically based strand highlight", |b| {
        let specular = b.parameter(
            kit::amount("specular_weight", 0.8, 0.0, 2.0)
                .with_group("strand")
                .with_label("Highlight"),
        );
        let tilt = b.parameter(
            kit::amount("shift", 0.035, -0.2, 0.2)
                .with_group("strand")
                .with_label("Cuticle tilt"),
        );
        let variation = b.parameter(
            kit::amount("shift_variation", 0.5, 0.0, 2.0)
                .with_group("strand")
                .with_label("Shift variation"),
        );
        let centred = b.subtract(&jitter, 0.5);
        let varied = b.multiply(centred, variation);
        let size = b.op(Op::Absolute, [tilt.clone().into()]);
        let varied = b.multiply(varied, size);
        let varied = b.multiply(varied, 2.0);
        let tilt = b.add(tilt, varied);
        let hair = kit::node(
            b,
            "bsdf.hair",
            [
                ("color", [0.0, 0.0, 0.0].into()),
                ("tangent", (&tangent).into()),
                ("roughness", (&mask.roughness).into()),
                ("shift", tilt.into()),
                ("specular", specular.into()),
                ("backlit", 0.0.into()),
                ("normal", (&normal).into()),
                ("occlusion", (&mask.occlusion).into()),
            ],
        );
        b.out(hair, "shader")
    });
    let surface = kit::add_shader(&mut b, &cel_shader, &strand);
    finish(b, surface, albedo.alpha, &albedo.rgb, 0.0006)
}

fn endfield_face(library: &Library) -> Result<Graph, Error> {
    let mut b = defined(library);
    let albedo = super::albedo(&mut b, [1.0, 0.88, 0.82, 1.0]);
    let normal = kit::normal(&mut b);
    let cel = cel(
        &mut b,
        &Look {
            shade: [0.88, 0.68, 0.68],
            edge: 0.5,
            softness: 0.06,
            terminator: [0.98, 0.52, 0.42],
            terminator_strength: 0.4,
            flatten: 0.5,
        },
    );
    let sampled = b.frame("Face shadow map", |b| {
        let texture = b.parameter(
            kit::texture("face_shadow_texture", TextureKind::Data)
                .with_group("face")
                .with_label("Face shadow (SDF)"),
        );
        let channel = b.parameter(
            kit::options("face_shadow_channel", &["r", "g", "b", "a"], 3)
                .with_group("face")
                .with_label("SDF channel"),
        );
        let found = kit::node(
            b,
            "npr.face_sample",
            [("texture", (&texture).into()), ("channel", channel.into())],
        );
        let bound = kit::value(b, "texture.bound", "value", [("texture", texture.into())]);
        (b.out(found, "sdf"), b.out(found, "mirrored"), bound)
    });
    let diffuse = b.frame("Face light (per light)", |b| {
        let softness = b.parameter(
            kit::amount("face_softness", 0.02, 0.0, 0.2)
                .with_group("face")
                .with_label("Shadow softness"),
        );
        let hair = b.parameter(
            kit::unit("hair_shadow", 0.8)
                .with_group("face")
                .with_label("Hair shadow"),
        );
        let (sdf, mirrored, bound) = sampled;
        let mapped = kit::value(
            b,
            "npr.face_sdf",
            "lit",
            [
                ("sdf", sdf.into()),
                ("mirrored", mirrored.into()),
                ("softness", (&softness).into()),
            ],
        );
        let drawn = kit::value(b, "npr.face_procedural", "lit", []);
        let face = b.lerp(drawn, mapped, bound);
        let fringe = kit::value(b, "npr.hair_shadow", "lit", []);
        let fringe = b.lerp(1.0, fringe, hair);
        let shadow = kit::value(b, "light.shadow", "visibility", []);
        let value = b.multiply(face, fringe);
        let value = b.multiply(value, shadow);
        let tone = banded(b, value, &cel);
        blocks::lit(b, &albedo.rgb, tone)
    });
    let rim = rim(&mut b, &normal, None);
    let flat = cel.flatten.clone();
    let ambient = b.frame("Ambient", |b| {
        blocks::ambient(
            b,
            [
                ("albedo", (&albedo.rgb).into()),
                ("normal", (&normal).into()),
                ("flatten", flat.into()),
            ],
        )
    });
    let light = b.add(diffuse, rim);
    let cel_shader = kit::lighting(&mut b, &light, &ambient);
    let weight = specular_weight(&mut b, 0.3);
    let roughness = b.parameter(
        kit::unit("specular_roughness", 0.55)
            .with_group("material")
            .with_label("Roughness"),
    );
    let gloss = blocks::specular_layer(
        &mut b,
        Gloss {
            albedo: &albedo.rgb,
            metalness: 0.0.into(),
            roughness: roughness.into(),
            weight: weight.into(),
            normal: (&normal).into(),
            occlusion: 1.0.into(),
        },
    );
    b.set(gloss.node(), "ior", 1.4);
    let surface = kit::add_shader(&mut b, &cel_shader, &gloss);
    finish(b, surface, albedo.alpha, &albedo.rgb, 0.0005)
}
