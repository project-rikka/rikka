use rikka_shader_graph::{
    Builder, Error, Function, FunctionPort, Graph, GraphKind, Input, Library, NodeKind, Op,
    ParameterDecl, ValueType, Wire,
};
use rikka_slang::{Hint, Literal, Role, TextureKind};

use super::kit;
use crate::{Family, Preset};

pub(super) fn presets() -> Vec<Preset> {
    let realistic = |key, title, summary, build| Preset {
        key,
        title,
        summary,
        family: Family::Realistic,
        build,
    };
    vec![
        realistic(
            "pbr",
            "Physically based",
            "Textured OpenPBR with glTF channels: base, metal-roughness, normal, occlusion, emission and opacity, plus coat, fuzz, transmission, subsurface and thin film.",
            pbr,
        ),
        realistic(
            "pbr_skin",
            "Skin",
            "Screen-space subsurface skin with a dual specular lobe, cavity, detail normal and thickness-driven back scatter.",
            pbr_skin,
        ),
        realistic(
            "pbr_hair",
            "Hair",
            "Marschner strands for hair cards drawn with alpha to coverage: flow and shift maps, tip tint, backlit glow.",
            pbr_hair,
        ),
        realistic(
            "pbr_eye",
            "Eye",
            "An eyeball under a refracting cornea: parallax iris, pupil, limbal ring and the caustic of light focused onto the iris.",
            pbr_eye,
        ),
        realistic(
            "pbr_cloth",
            "Cloth",
            "Woven fabric: a rough base under microfibre sheen, anisotropic for silk and satin.",
            pbr_cloth,
        ),
        realistic(
            "pbr_glass",
            "Glass",
            "Clear refracting glass with dispersion and absorption over its thickness; draw it transparent.",
            pbr_glass,
        ),
    ]
}

#[derive(Clone, Copy)]
pub(super) struct Lobes {
    pub(super) anisotropy: bool,
    pub(super) transmission: bool,
    pub(super) subsurface: bool,
    pub(super) coat: bool,
    pub(super) fuzz: bool,
    pub(super) thin_film: bool,
}

impl Lobes {
    pub(super) const ALL: Self = Self {
        anisotropy: true,
        transmission: true,
        subsurface: true,
        coat: true,
        fuzz: true,
        thin_film: true,
    };

    pub(super) const NONE: Self = Self {
        anisotropy: false,
        transmission: false,
        subsurface: false,
        coat: false,
        fuzz: false,
        thin_film: false,
    };
}

pub(super) struct Tuning {
    defaults: &'static [(&'static str, Literal)],
    presets: &'static [(&'static str, &'static str, Literal)],
}

pub(super) const PLAIN: Tuning = Tuning {
    defaults: &[],
    presets: &[],
};

const CLOTH: Tuning = Tuning {
    defaults: &[
        ("base_diffuse_roughness", Literal::Float(0.5)),
        ("specular_weight", Literal::Float(0.5)),
        ("specular_roughness", Literal::Float(0.7)),
        ("fuzz_weight", Literal::Float(0.6)),
    ],
    presets: &[
        (
            "base_color",
            "velvet",
            Literal::Vec4([0.18, 0.03, 0.06, 1.0]),
        ),
        ("base_color", "silk", Literal::Vec4([0.55, 0.08, 0.12, 1.0])),
        ("base_color", "satin", Literal::Vec4([0.08, 0.1, 0.3, 1.0])),
        ("specular_weight", "velvet", Literal::Float(0.4)),
        ("specular_weight", "silk", Literal::Float(1.0)),
        ("specular_weight", "satin", Literal::Float(1.0)),
        ("specular_color", "silk", Literal::Vec3([1.0, 0.8, 0.82])),
        ("specular_roughness", "velvet", Literal::Float(0.8)),
        ("specular_roughness", "silk", Literal::Float(0.35)),
        ("specular_roughness", "satin", Literal::Float(0.3)),
        ("specular_roughness_anisotropy", "silk", Literal::Float(0.8)),
        (
            "specular_roughness_anisotropy",
            "satin",
            Literal::Float(0.6),
        ),
        ("fuzz_weight", "velvet", Literal::Float(1.0)),
        ("fuzz_weight", "silk", Literal::Float(0.25)),
        ("fuzz_weight", "satin", Literal::Float(0.4)),
        ("fuzz_color", "velvet", Literal::Vec3([0.75, 0.2, 0.3])),
        ("fuzz_color", "silk", Literal::Vec3([1.0, 0.85, 0.85])),
        ("fuzz_color", "satin", Literal::Vec3([0.5, 0.6, 1.0])),
        ("fuzz_roughness", "velvet", Literal::Float(0.35)),
        ("fuzz_roughness", "silk", Literal::Float(0.6)),
        ("fuzz_roughness", "satin", Literal::Float(0.45)),
    ],
};

const GLASS: Tuning = Tuning {
    defaults: &[
        ("base_color", Literal::Vec4([1.0, 1.0, 1.0, 1.0])),
        ("specular_roughness", Literal::Float(0.0)),
        ("transmission_weight", Literal::Float(1.0)),
        ("transmission_dispersion_scale", Literal::Float(0.5)),
        ("geometry_thickness", Literal::Float(0.05)),
    ],
    presets: &[],
};

fn declare(builder: &mut Builder<'_>, tuning: &Tuning, mut parameter: ParameterDecl) -> Wire {
    if let Some((_, value)) = tuning
        .defaults
        .iter()
        .find(|(name, _)| *name == parameter.name)
    {
        parameter = parameter.with_default(*value);
    }
    parameter.presets.extend(
        tuning
            .presets
            .iter()
            .filter(|(name, _, _)| *name == parameter.name)
            .map(|(_, preset, value)| kit::preset(preset, *value)),
    );
    builder.parameter(parameter)
}

pub(super) struct Physical {
    pub(super) shader: Wire,
    pub(super) alpha: Wire,
    pub(super) albedo: Wire,
    pub(super) metalness: Wire,
    pub(super) roughness: Wire,
    pub(super) normal: Wire,
    pub(super) occlusion: Wire,
    pub(super) emission: Wire,
    pub(super) specular: Wire,
}

pub(super) fn openpbr(builder: &mut Builder<'_>, lobes: Lobes, tuning: &Tuning) -> Physical {
    let (albedo, alpha, diffuse_roughness) = builder.frame("Base", |b| {
        let color = declare(b, tuning, super::base_color([0.8, 0.8, 0.8, 1.0]));
        let texture = declare(b, tuning, super::base_texture());
        let weight = declare(
            b,
            tuning,
            kit::unit("base_weight", 1.0)
                .with_group("base")
                .with_label("Weight")
                .with_role(Role::AlbedoWeight),
        );
        let diffuse_roughness = declare(
            b,
            tuning,
            kit::unit("base_diffuse_roughness", 0.0)
                .with_group("base")
                .with_label("Diffuse roughness"),
        );
        let texel = kit::rgba(b, &texture);
        let tinted = b.multiply(color, texel);
        let vertex = b.input(Input::VertexColor);
        let shaded = b.multiply(&tinted, vertex);
        let rgb = kit::channel(b, &shaded, "xyz");
        let albedo = b.multiply(rgb, weight);
        let alpha = kit::channel(b, &tinted, "w");
        (albedo, alpha, diffuse_roughness)
    });
    let (metalness, roughness) = builder.frame("Metal and roughness", |b| {
        let metalness = declare(
            b,
            tuning,
            kit::unit("base_metalness", 0.0)
                .with_group("base")
                .with_label("Metalness")
                .with_role(Role::Metalness),
        );
        let texture = declare(
            b,
            tuning,
            kit::texture("metal_roughness_texture", TextureKind::Data)
                .with_group("base")
                .with_label("Metal-roughness texture")
                .with_role(Role::MetalnessTexture),
        );
        let roughness = declare(
            b,
            tuning,
            kit::unit("specular_roughness", 0.3)
                .with_group("specular")
                .with_label("Roughness"),
        );
        let texel = kit::data(b, &texture);
        let blue = kit::channel(b, &texel, "z");
        let green = kit::channel(b, &texel, "y");
        (b.multiply(metalness, blue), b.multiply(roughness, green))
    });
    let normal = super::mapped_normal(builder);
    let occlusion = builder.frame("Occlusion", |b| {
        let texture = declare(
            b,
            tuning,
            kit::texture("occlusion_texture", TextureKind::Data)
                .with_group("geometry")
                .with_label("Occlusion texture"),
        );
        let strength = declare(
            b,
            tuning,
            kit::unit("occlusion_strength", 1.0)
                .with_group("geometry")
                .with_label("Occlusion strength"),
        );
        let texel = kit::data(b, &texture);
        let red = kit::channel(b, &texel, "x");
        b.lerp(1.0, red, strength)
    });
    let emission = {
        let colored = super::emitted(builder);
        builder.frame("Emission", |b| {
            let luminance = declare(
                b,
                tuning,
                kit::amount("emission_luminance", kit::NITS_PER_UNIT, 0.0, 100_000.0)
                    .with_group("emission")
                    .with_label("Luminance")
                    .with_hint(Hint::Logarithmic)
                    .with_role(Role::EmissionLuminance),
            );
            let scaled = kit::divide(b, luminance, kit::NITS_PER_UNIT);
            b.multiply(colored, scaled)
        })
    };
    let surface = builder.node(NodeKind::Extern("bsdf.principled".to_owned()));
    builder
        .set(surface, "base_color", &albedo)
        .set(surface, "metalness", &metalness)
        .set(surface, "roughness", &roughness)
        .set(surface, "diffuse_roughness", diffuse_roughness)
        .set(surface, "normal", &normal)
        .set(surface, "occlusion", &occlusion)
        .set(surface, "emission", &emission);
    let specular = builder.frame("Specular", |b| {
        let weight = declare(
            b,
            tuning,
            kit::amount("specular_weight", 1.0, 0.0, 1.0)
                .with_group("specular")
                .with_label("Weight"),
        );
        let color = declare(
            b,
            tuning,
            kit::color("specular_color", [1.0, 1.0, 1.0])
                .with_group("specular")
                .with_label("Colour"),
        );
        let ior = declare(
            b,
            tuning,
            kit::amount("specular_ior", 1.5, 1.0, 3.0)
                .with_group("specular")
                .with_label("IOR"),
        );
        b.set(surface, "specular_weight", &weight)
            .set(surface, "specular_color", color)
            .set(surface, "ior", ior);
        if lobes.anisotropy {
            let anisotropy = declare(
                b,
                tuning,
                kit::unit("specular_roughness_anisotropy", 0.0)
                    .with_group("specular")
                    .with_label("Anisotropy"),
            );
            let rotation = declare(
                b,
                tuning,
                ParameterDecl::float("specular_rotation", 0.0)
                    .with_group("specular")
                    .with_label("Anisotropy rotation")
                    .with_hint(Hint::Angle),
            );
            let x = b.op(Op::Cosine, [rotation.clone().into()]);
            let y = b.op(Op::Sine, [rotation.into()]);
            let direction = kit::combine(b, [x.into(), y.into(), 0.0.into()]);
            let tangent = kit::value(
                b,
                "vector.from_tangent",
                "value",
                [("vector", direction.into()), ("normal", (&normal).into())],
            );
            b.set(surface, "anisotropy", anisotropy)
                .set(surface, "tangent", tangent);
        }
        weight
    });
    if lobes.transmission {
        builder.frame("Transmission", |b| {
            let weight = declare(
                b,
                tuning,
                kit::unit("transmission_weight", 0.0)
                    .with_group("transmission")
                    .with_label("Weight"),
            );
            let color = declare(
                b,
                tuning,
                kit::color("transmission_color", [1.0, 1.0, 1.0])
                    .with_group("transmission")
                    .with_label("Colour"),
            );
            let depth = declare(
                b,
                tuning,
                kit::amount("transmission_depth", 0.0, 0.0, 1.0)
                    .with_group("transmission")
                    .with_label("Depth"),
            );
            let scale = declare(
                b,
                tuning,
                kit::unit("transmission_dispersion_scale", 0.0)
                    .with_group("transmission")
                    .with_label("Dispersion scale"),
            );
            let abbe = declare(
                b,
                tuning,
                kit::amount("transmission_dispersion_abbe_number", 20.0, 9.0, 91.0)
                    .with_group("transmission")
                    .with_label("Abbe number")
                    .with_hint(Hint::Advanced),
            );
            let abbe = kit::maximum(b, abbe, 1.0);
            let dispersion = kit::divide(b, scale, abbe);
            let thickness = declare(
                b,
                tuning,
                kit::amount("geometry_thickness", 0.0, 0.0, 0.5)
                    .with_group("geometry")
                    .with_label("Thickness"),
            );
            let texture = declare(
                b,
                tuning,
                kit::texture("thickness_texture", TextureKind::Data)
                    .with_group("geometry")
                    .with_label("Thickness texture"),
            );
            let texel = kit::data(b, &texture);
            let green = kit::channel(b, &texel, "y");
            let thickness = b.multiply(thickness, green);
            b.set(surface, "transmission_weight", weight)
                .set(surface, "transmission_color", color)
                .set(surface, "transmission_depth", depth)
                .set(surface, "dispersion", dispersion)
                .set(surface, "thickness", thickness);
        });
    }
    if lobes.transmission || lobes.subsurface {
        builder.frame("Thin walls", |b| {
            let thin = declare(
                b,
                tuning,
                kit::toggle("geometry_thin_walled", false)
                    .with_group("geometry")
                    .with_label("Thin walled"),
            );
            b.set(surface, "thin_walled", thin);
        });
    }
    if lobes.subsurface {
        builder.frame("Subsurface", |b| {
            let weight = declare(
                b,
                tuning,
                kit::unit("subsurface_weight", 0.0)
                    .with_group("subsurface")
                    .with_label("Weight"),
            );
            let color = declare(
                b,
                tuning,
                kit::color("subsurface_color", [0.8, 0.8, 0.8])
                    .with_group("subsurface")
                    .with_label("Colour"),
            );
            let radius = declare(
                b,
                tuning,
                kit::amount("subsurface_radius", 1.0, 0.0, 1.0)
                    .with_group("subsurface")
                    .with_label("Radius")
                    .with_hint(Hint::Logarithmic),
            );
            let scale = declare(
                b,
                tuning,
                kit::vector3("subsurface_radius_scale", [1.0, 0.5, 0.25])
                    .with_group("subsurface")
                    .with_label("Radius scale"),
            );
            let anisotropy = declare(
                b,
                tuning,
                ParameterDecl::float("subsurface_scatter_anisotropy", 0.0)
                    .with_range(-1.0, 1.0)
                    .with_group("subsurface")
                    .with_label("Anisotropy"),
            );
            let radius = b.multiply(radius, scale);
            b.set(surface, "subsurface_weight", weight)
                .set(surface, "subsurface_color", color)
                .set(surface, "subsurface_radius", radius)
                .set(surface, "subsurface_anisotropy", anisotropy);
        });
    }
    if lobes.coat {
        builder.frame("Coat", |b| {
            let ports = [
                (
                    "coat_weight",
                    kit::unit("coat_weight", 0.0).with_label("Weight"),
                ),
                (
                    "coat_color",
                    kit::color("coat_color", [1.0, 1.0, 1.0]).with_label("Colour"),
                ),
                (
                    "coat_roughness",
                    kit::unit("coat_roughness", 0.0).with_label("Roughness"),
                ),
                (
                    "coat_anisotropy",
                    kit::unit("coat_roughness_anisotropy", 0.0).with_label("Anisotropy"),
                ),
                (
                    "coat_ior",
                    kit::amount("coat_ior", 1.6, 1.0, 3.0).with_label("IOR"),
                ),
                (
                    "coat_darkening",
                    kit::unit("coat_darkening", 1.0).with_label("Darkening"),
                ),
            ];
            for (port, parameter) in ports {
                let wire = declare(b, tuning, parameter.with_group("coat"));
                b.set(surface, port, wire);
            }
        });
    }
    if lobes.fuzz {
        builder.frame("Fuzz", |b| {
            let ports = [
                (
                    "fuzz_weight",
                    kit::unit("fuzz_weight", 0.0).with_label("Weight"),
                ),
                (
                    "fuzz_color",
                    kit::color("fuzz_color", [1.0, 1.0, 1.0]).with_label("Colour"),
                ),
                (
                    "fuzz_roughness",
                    kit::unit("fuzz_roughness", 0.5).with_label("Roughness"),
                ),
            ];
            for (port, parameter) in ports {
                let wire = declare(b, tuning, parameter.with_group("fuzz"));
                b.set(surface, port, wire);
            }
        });
    }
    if lobes.thin_film {
        builder.frame("Thin film", |b| {
            let ports = [
                (
                    "thin_film_weight",
                    kit::unit("thin_film_weight", 0.0).with_label("Weight"),
                ),
                (
                    "thin_film_thickness",
                    kit::amount("thin_film_thickness", 0.5, 0.0, 1.0).with_label("Thickness"),
                ),
                (
                    "thin_film_ior",
                    kit::amount("thin_film_ior", 1.4, 1.0, 3.0).with_label("IOR"),
                ),
            ];
            for (port, parameter) in ports {
                let wire = declare(b, tuning, parameter.with_group("thin_film"));
                b.set(surface, port, wire);
            }
        });
    }
    Physical {
        shader: builder.out(surface, "shader"),
        alpha,
        albedo,
        metalness,
        roughness,
        specular,
        normal,
        occlusion,
        emission,
    }
}

fn surface(lobes: Lobes, tuning: &Tuning, library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    let physical = openpbr(&mut builder, lobes, tuning);
    builder.output([
        ("surface", physical.shader.into()),
        ("alpha", physical.alpha.into()),
    ]);
    builder.finish()
}

fn pbr(library: &Library) -> Result<Graph, Error> {
    surface(Lobes::ALL, &PLAIN, library)
}

fn pbr_cloth(library: &Library) -> Result<Graph, Error> {
    let lobes = Lobes {
        anisotropy: true,
        fuzz: true,
        ..Lobes::NONE
    };
    surface(lobes, &CLOTH, library)
}

fn pbr_glass(library: &Library) -> Result<Graph, Error> {
    let lobes = Lobes {
        transmission: true,
        ..Lobes::NONE
    };
    surface(lobes, &GLASS, library)
}

fn shaded_base(builder: &mut Builder<'_>, value: [f32; 4]) -> (Wire, Wire) {
    builder.frame("Base", |b| {
        let color = b.parameter(super::base_color(value));
        let texture = b.parameter(super::base_texture());
        let texel = kit::rgba(b, &texture);
        let tinted = b.multiply(color, texel);
        let vertex = b.input(Input::VertexColor);
        let shaded = b.multiply(&tinted, vertex);
        (
            kit::channel(b, &shaded, "xyz"),
            kit::channel(b, &tinted, "w"),
        )
    })
}

fn group(parameter: ParameterDecl, group: &str, label: &str) -> ParameterDecl {
    parameter.with_group(group).with_label(label)
}

fn pbr_skin(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    let (albedo, alpha) = shaded_base(&mut builder, [0.8, 0.6, 0.5, 1.0]);
    let skin = builder.node(NodeKind::Extern("bsdf.skin".to_owned()));
    builder.set(skin, "color", &albedo);
    builder.frame("Subsurface", |b| {
        let weight = b.parameter(group(
            kit::unit("subsurface_weight", 1.0),
            "subsurface",
            "Weight",
        ));
        let tint = b.parameter(group(
            kit::color("subsurface_tint", [1.0, 1.0, 1.0]),
            "subsurface",
            "Tint",
        ));
        let radius = b.parameter(group(
            kit::amount("subsurface_radius", 0.004, 0.0, 0.02).with_hint(Hint::Logarithmic),
            "subsurface",
            "Radius",
        ));
        let scale = b.parameter(group(
            kit::vector3("subsurface_radius_scale", [1.0, 0.37, 0.19]),
            "subsurface",
            "Radius scale",
        ));
        let anisotropy = b.parameter(group(
            ParameterDecl::float("subsurface_scatter_anisotropy", 0.0).with_range(-1.0, 1.0),
            "subsurface",
            "Anisotropy",
        ));
        let thickness = b.parameter(group(
            kit::amount("thickness", 0.0, 0.0, 0.05),
            "subsurface",
            "Thickness",
        ));
        let thickness_texture = b.parameter(group(
            kit::texture("thickness_texture", TextureKind::Data),
            "subsurface",
            "Thickness texture",
        ));
        let tinted = b.multiply(&albedo, tint);
        let color = b.saturate(tinted);
        let radius = b.multiply(radius, scale);
        let texel = kit::data(b, &thickness_texture);
        let green = kit::channel(b, &texel, "y");
        let thickness = b.multiply(thickness, green);
        b.set(skin, "subsurface_weight", weight)
            .set(skin, "subsurface_color", color)
            .set(skin, "subsurface_radius", radius)
            .set(skin, "subsurface_anisotropy", anisotropy)
            .set(skin, "thickness", thickness);
    });
    let cavity = builder.frame("Specular", |b| {
        let weight = b.parameter(group(
            kit::amount("specular_weight", 1.0, 0.0, 1.0),
            "specular",
            "Weight",
        ));
        let roughness = b.parameter(group(
            kit::unit("specular_roughness", 0.5),
            "specular",
            "Roughness",
        ));
        let roughness_texture = b.parameter(group(
            kit::texture("roughness_texture", TextureKind::Data),
            "specular",
            "Roughness texture",
        ));
        let second = b.parameter(group(
            kit::unit("specular_second_roughness", 0.3),
            "specular",
            "Second roughness",
        ));
        let mix = b.parameter(group(
            kit::unit("specular_lobe_mix", 0.15),
            "specular",
            "Second lobe",
        ));
        let ior = b.parameter(group(
            kit::amount("specular_ior", 1.4, 1.0, 2.0),
            "specular",
            "IOR",
        ));
        let cavity_texture = b.parameter(group(
            kit::texture("cavity_texture", TextureKind::Data),
            "specular",
            "Cavity texture",
        ));
        let cavity_strength = b.parameter(group(
            kit::unit("cavity_strength", 1.0),
            "specular",
            "Cavity strength",
        ));
        let diffuse_roughness = b.parameter(group(
            kit::unit("base_diffuse_roughness", 0.0),
            "base",
            "Diffuse roughness",
        ));
        let texel = kit::data(b, &roughness_texture);
        let rough = kit::channel(b, &texel, "y");
        let roughness = b.multiply(roughness, &rough);
        let second = b.multiply(second, rough);
        let texel = kit::data(b, &cavity_texture);
        let red = kit::channel(b, &texel, "x");
        let cavity = b.lerp(1.0, red, cavity_strength);
        b.set(skin, "specular_weight", weight)
            .set(skin, "roughness", roughness)
            .set(skin, "second_roughness", second)
            .set(skin, "second_lobe", mix)
            .set(skin, "ior", ior)
            .set(skin, "cavity", &cavity)
            .set(skin, "diffuse_roughness", diffuse_roughness);
        cavity
    });
    builder.frame("Fuzz", |b| {
        let weight = b.parameter(group(kit::unit("fuzz_weight", 0.0), "fuzz", "Weight"));
        let color = b.parameter(group(
            kit::color("fuzz_color", [1.0, 1.0, 1.0]),
            "fuzz",
            "Colour",
        ));
        let roughness = b.parameter(group(kit::unit("fuzz_roughness", 0.6), "fuzz", "Roughness"));
        b.set(skin, "fuzz_weight", weight)
            .set(skin, "fuzz_color", color)
            .set(skin, "fuzz_roughness", roughness);
    });
    builder.frame("Normal and detail", |b| {
        let texture = b.parameter(super::normal_texture());
        let scale = b.parameter(super::normal_scale());
        let detail_texture = b.parameter(group(
            kit::texture("detail_normal_texture", TextureKind::Normal),
            "geometry",
            "Detail normal map",
        ));
        let detail_scale = b.parameter(group(
            kit::amount("detail_scale", 8.0, 1.0, 64.0),
            "geometry",
            "Detail tiling",
        ));
        let detail_strength = b.parameter(group(
            kit::amount("detail_strength", 0.5, 0.0, 2.0),
            "geometry",
            "Detail strength",
        ));
        let base = kit::sample(b, TextureKind::Normal, &texture);
        let base = b.out(base, "normal");
        let flat = kit::combine(b, [(&scale).into(), scale.into(), 1.0.into()]);
        let base = b.multiply(base, flat);
        let base = kit::normalize(b, base);
        let at = kit::uv(b);
        let at = b.multiply(at, detail_scale);
        let detail = kit::sample_at(b, TextureKind::Normal, &detail_texture, at);
        let detail = b.out(detail, "normal");
        let flat = kit::combine(
            b,
            [
                (&detail_strength).into(),
                detail_strength.into(),
                1.0.into(),
            ],
        );
        let detail = b.multiply(detail, flat);
        let detail = kit::normalize(b, detail);
        let base_xy = kit::channel(b, &base, "xy");
        let detail_xy = kit::channel(b, &detail, "xy");
        let xy = b.add(base_xy, detail_xy);
        let base_z = kit::channel(b, &base, "z");
        let detail_z = kit::channel(b, &detail, "z");
        let z = b.multiply(base_z, detail_z);
        let x = kit::channel(b, &xy, "x");
        let y = kit::channel(b, &xy, "y");
        let joined = kit::combine(b, [x.into(), y.into(), z.into()]);
        let joined = kit::normalize(b, joined);
        let normal = b.op(Op::NormalMap, [joined.into(), 1.0.into()]);
        b.set(skin, "normal", normal);
    });
    builder.frame("Occlusion", |b| {
        let texture = b.parameter(group(
            kit::texture("occlusion_texture", TextureKind::Data),
            "geometry",
            "Occlusion texture",
        ));
        let strength = b.parameter(group(
            kit::unit("occlusion_strength", 1.0),
            "geometry",
            "Occlusion strength",
        ));
        let texel = kit::data(b, &texture);
        let red = kit::channel(b, &texel, "x");
        let occlusion = b.lerp(1.0, red, strength);
        let cavity = b.lerp(1.0, cavity, 0.5);
        let occlusion = b.multiply(occlusion, cavity);
        b.set(skin, "occlusion", occlusion);
    });
    let shader = builder.out(skin, "shader");
    builder.output([("surface", shader.into()), ("alpha", alpha.into())]);
    builder.finish()
}

fn pbr_hair(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    let hair = builder.node(NodeKind::Extern("bsdf.hair".to_owned()));
    let direction = builder.parameter(group(
        kit::options("direction", &["u", "v"], 1),
        "strand",
        "Strand direction",
    ));
    let alpha = builder.frame("Base and tip", |b| {
        let color = b.parameter(kit::presets(
            super::base_color([0.32, 0.2, 0.12, 1.0]),
            &[
                ("blonde", Literal::Vec4([0.85, 0.62, 0.38, 1.0])),
                ("brown", Literal::Vec4([0.32, 0.2, 0.12, 1.0])),
                ("black", Literal::Vec4([0.04, 0.035, 0.03, 1.0])),
                ("red", Literal::Vec4([0.6, 0.18, 0.08, 1.0])),
            ],
        ));
        let texture = b.parameter(super::base_texture());
        let tip = b.parameter(group(
            kit::color("tip_color", [1.0, 1.0, 1.0]),
            "base",
            "Tip tint",
        ));
        let texel = kit::rgba(b, &texture);
        let base = b.multiply(color, texel);
        let at = kit::uv(b);
        let u = kit::channel(b, &at, "x");
        let v = kit::channel(b, &at, "y");
        let along = b.lerp(u, v, &direction);
        let along = b.saturate(along);
        let tint = b.lerp([1.0, 1.0, 1.0], tip, along);
        let rgb = kit::channel(b, &base, "xyz");
        let albedo = b.multiply(rgb, tint);
        b.set(hair, "color", albedo);
        kit::channel(b, &base, "w")
    });
    builder.frame("Strand flow", |b| {
        let flow_texture = b.parameter(group(
            kit::texture("flow_texture", TextureKind::Data),
            "strand",
            "Flow map",
        ));
        let along = b.lerp([1.0, 0.0], [0.0, 1.0], &direction);
        let texel = kit::data(b, &flow_texture);
        let painted = kit::channel(b, &texel, "xy");
        let painted = b.multiply(painted, 2.0);
        let painted = b.subtract(painted, 1.0);
        let bound = kit::value(
            b,
            "texture.bound",
            "value",
            [("texture", flow_texture.into())],
        );
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
        let tangent = kit::normalize(b, tangent);
        b.set(hair, "tangent", tangent);
    });
    builder.frame("Strand", |b| {
        let roughness = b.parameter(group(kit::unit("roughness", 0.35), "strand", "Roughness"));
        let shift = b.parameter(group(
            kit::amount("shift", 0.035, -0.2, 0.2),
            "strand",
            "Cuticle tilt",
        ));
        let shift_texture = b.parameter(group(
            kit::texture("shift_texture", TextureKind::Data),
            "strand",
            "Shift map",
        ));
        let variation = b.parameter(group(
            kit::amount("shift_variation", 0.5, 0.0, 2.0),
            "strand",
            "Shift variation",
        ));
        let specular = b.parameter(group(
            kit::amount("specular", 1.0, 0.0, 2.0),
            "strand",
            "Specular",
        ));
        let backlit = b.parameter(group(
            kit::amount("backlit", 1.0, 0.0, 2.0),
            "strand",
            "Backlit",
        ));
        let scatter = b.parameter(group(kit::unit("scatter", 0.4), "strand", "Scatter"));
        let texel = kit::data(b, &shift_texture);
        let red = kit::channel(b, &texel, "x");
        let centred = b.subtract(red, 0.5);
        let varied = b.multiply(centred, variation);
        let size = b.op(Op::Absolute, [shift.clone().into()]);
        let varied = b.multiply(varied, size);
        let varied = b.multiply(varied, 2.0);
        let shift = b.add(shift, varied);
        b.set(hair, "roughness", roughness)
            .set(hair, "shift", shift)
            .set(hair, "specular", specular)
            .set(hair, "backlit", backlit)
            .set(hair, "scatter", scatter);
    });
    builder.frame("Occlusion and coverage", |b| {
        let texture = b.parameter(group(
            kit::texture("occlusion_texture", TextureKind::Data),
            "geometry",
            "Occlusion texture",
        ));
        let strength = b.parameter(group(
            kit::unit("occlusion_strength", 1.0),
            "geometry",
            "Occlusion strength",
        ));
        let texel = kit::data(b, &texture);
        let red = kit::channel(b, &texel, "x");
        let occlusion = b.lerp(1.0, red, strength);
        let vertex = b.input(Input::VertexColor);
        let coverage = kit::channel(b, &vertex, "w");
        b.set(hair, "occlusion", occlusion)
            .set(hair, "alpha", coverage);
    });
    let shader = builder.out(hair, "shader");
    builder.output([("surface", shader.into()), ("alpha", alpha.into())]);
    builder.finish()
}

const IRIS: &str = "Iris refraction";

fn iris_function(builder: &mut Builder<'_>) {
    let float = |name: &str| FunctionPort::new(name, ValueType::Float);
    let plane = |name: &str| FunctionPort::new(name, ValueType::Float2);
    let space = |name: &str| FunctionPort::new(name, ValueType::Float3);
    let function = Function {
        inputs: vec![
            plane("uv"),
            plane("center"),
            plane("radius"),
            float("softness"),
            float("depth"),
            float("ior"),
            float("bulge"),
            space("cornea"),
            space("normal"),
            space("view"),
        ],
        outputs: vec![
            plane("iris_uv"),
            float("radial"),
            float("mask"),
            space("forward"),
            space("cornea"),
        ],
        doc: "Where the view ray, bent by the cornea, meets an iris sunk depth behind the eye's surface. uv, center and radius place the iris disc in uv space (softness feathers its edge); cornea is the normal-mapped eye surface, bulged by bulge over the iris; ior is the cornea's index. Outputs: iris_uv to read the iris texture at, radial the distance from the iris centre in iris radii (1 at the limbus), mask 1 over the iris, forward the direction the iris faces, cornea the bulged cornea normal.".to_owned(),
        body: Default::default(),
    };
    builder.function(IRIS, function, |b| {
        let at = b.function_input("uv");
        let center = b.function_input("center");
        let radius = b.function_input("radius");
        let softness = b.function_input("softness");
        let depth = b.function_input("depth");
        let ior = b.function_input("ior");
        let bulge = b.function_input("bulge");
        let cornea = b.function_input("cornea");
        let normal = b.function_input("normal");
        let view = b.function_input("view");

        let offset = b.subtract(at, &center);
        let reach = kit::maximum(b, &radius, [1e-4, 1e-4]);
        let offset = kit::divide(b, offset, reach);
        let distance = b.op(Op::Length, [offset.clone().into()]);
        let inner = kit::one_minus(b, softness);
        let edge = b.smoothstep(inner, 1.0, &distance);
        let mask = kit::one_minus(b, edge);
        let x = kit::channel(b, &offset, "x");
        let y = kit::channel(b, &offset, "y");
        let local = kit::combine(b, [x.into(), y.into(), 0.0.into()]);
        let sideways = kit::value(
            b,
            "vector.from_tangent",
            "value",
            [("vector", local.into()), ("normal", (&normal).into())],
        );
        let lean = b.multiply(&mask, 0.45);
        let sideways = b.multiply(sideways, lean);
        let forward = b.subtract(&normal, sideways);
        let forward = kit::normalize(b, forward);

        let squared = b.multiply(&distance, &distance);
        let dome = kit::one_minus(b, &squared);
        let lift = b.multiply(bulge, &mask);
        let lift = b.multiply(lift, &dome);
        let pushed = b.multiply(&forward, lift);
        let pushed = b.add(&cornea, pushed);
        let pushed = kit::normalize(b, pushed);
        let bulged = b.lerp(cornea, pushed, &mask);
        let bulged = kit::normalize(b, bulged);

        let incoming = b.op(Op::Negate, [view.into()]);
        let ior = kit::maximum(b, ior, 1.0);
        let eta = b.op(Op::Reciprocal, [ior.into()]);
        let bent = b.op(Op::Refract, [incoming.into(), (&bulged).into(), eta.into()]);
        let toward = b.op(Op::Negate, [bent.clone().into()]);
        let facing = b.dot(toward, &forward);
        let facing = kit::maximum(b, facing, 0.15);
        let across = kit::value(
            b,
            "vector.to_tangent",
            "value",
            [("vector", bent.into()), ("normal", normal.into())],
        );
        let across = kit::channel(b, &across, "xy");
        let across = kit::divide(b, across, facing);
        let height = kit::maximum(b, dome, 0.0);
        let height = b.multiply(depth, height);
        let shift = b.multiply(across, height);
        let shifted = b.add(offset, shift);
        let radial = b.op(Op::Length, [shifted.clone().into()]);
        let scaled = b.multiply(shifted, radius);
        let iris_uv = b.add(center, scaled);
        b.function_output("iris_uv", iris_uv)
            .function_output("radial", radial)
            .function_output("mask", mask)
            .function_output("forward", forward)
            .function_output("cornea", bulged);
    });
}

fn pbr_eye(library: &Library) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    iris_function(&mut builder);
    let eye = builder.node(NodeKind::Extern("bsdf.eye".to_owned()));
    let (sclera, alpha) = builder.frame("Sclera", |b| {
        let color =
            b.parameter(super::base_color([0.9, 0.88, 0.86, 1.0]).with_label("Sclera colour"));
        let texture = b.parameter(super::base_texture().with_label("Sclera texture"));
        let scatter = b.parameter(group(kit::unit("sclera_scatter", 0.5), "base", "Scatter"));
        let texel = kit::rgba(b, &texture);
        let base = b.multiply(color, texel);
        b.set(eye, "sclera_scatter", scatter);
        (kit::channel(b, &base, "xyz"), kit::channel(b, &base, "w"))
    });
    let cornea = builder.frame("Cornea", |b| {
        let ior = b.parameter(group(
            kit::amount("cornea_ior", 1.376, 1.0, 2.0),
            "cornea",
            "IOR",
        ));
        let roughness = b.parameter(group(
            kit::unit("cornea_roughness", 0.04),
            "cornea",
            "Roughness",
        ));
        let texture = b.parameter(super::normal_texture());
        let scale = b.parameter(super::normal_scale());
        let texel = kit::sample(b, TextureKind::Normal, &texture);
        let tangent = b.out(texel, "normal");
        let mapped = b.op(Op::NormalMap, [tangent.into(), scale.into()]);
        b.set(eye, "cornea_ior", &ior)
            .set(eye, "cornea_roughness", roughness);
        (mapped, ior)
    });
    builder.frame("Iris", |b| {
        let color = b.parameter(kit::presets(
            group(
                kit::color("iris_color", [0.25, 0.4, 0.55]),
                "iris",
                "Colour",
            ),
            &[
                ("brown", Literal::Vec3([0.35, 0.2, 0.1])),
                ("green", Literal::Vec3([0.25, 0.42, 0.22])),
                ("blue", Literal::Vec3([0.25, 0.4, 0.55])),
                ("grey", Literal::Vec3([0.45, 0.48, 0.5])),
            ],
        ));
        let texture = b.parameter(group(
            kit::texture("iris_texture", TextureKind::Colour),
            "iris",
            "Texture",
        ));
        let center = b.parameter(group(
            kit::vector2("iris_center", [0.5, 0.5]),
            "iris",
            "Centre",
        ));
        let radius = b.parameter(group(
            kit::vector2("iris_radius", [0.15, 0.15]),
            "iris",
            "Radius",
        ));
        let pupil = b.parameter(group(kit::unit("pupil", 0.35), "iris", "Pupil"));
        let softness = b.parameter(group(
            ParameterDecl::float("iris_softness", 0.06).with_range(0.0, 0.5),
            "iris",
            "Edge softness",
        ));
        let depth = b.parameter(group(kit::unit("iris_depth", 0.35), "iris", "Depth"));
        let limbal_width = b.parameter(group(
            ParameterDecl::float("limbal_width", 0.12).with_range(0.0, 0.5),
            "iris",
            "Limbal ring width",
        ));
        let limbal_darkness = b.parameter(group(
            kit::unit("limbal_darkness", 0.6),
            "iris",
            "Limbal ring darkness",
        ));
        let caustic = b.parameter(group(
            kit::amount("caustic", 2.0, 0.0, 8.0),
            "iris",
            "Caustic",
        ));
        let bulge = b.parameter(group(kit::unit("cornea_bulge", 0.5), "cornea", "Bulge"));
        let normal = kit::normal(b);
        let view = kit::view(b);
        let at = kit::uv(b);
        let (mapped, ior) = cornea;
        let refraction = b.call(
            IRIS,
            [
                ("uv", at.into()),
                ("center", center.into()),
                ("radius", radius.into()),
                ("softness", softness.into()),
                ("depth", depth.into()),
                ("ior", ior.into()),
                ("bulge", bulge.into()),
                ("cornea", mapped.into()),
                ("normal", normal.into()),
                ("view", view.into()),
            ],
        );
        let iris_uv = b.out(refraction, "iris_uv");
        let radial = b.out(refraction, "radial");
        let mask = b.out(refraction, "mask");
        let texel = kit::sample_at(b, TextureKind::Colour, &texture, iris_uv);
        let texel = b.out(texel, "rgb");
        let iris = b.multiply(color, texel);
        let low = b.subtract(&pupil, 0.04);
        let high = b.add(pupil, 0.04);
        let opening = b.smoothstep(low, high, &radial);
        let dark = kit::one_minus(b, opening);
        let iris = b.lerp(iris, [0.01, 0.01, 0.01], dark);
        let inner = kit::one_minus(b, limbal_width);
        let ring = b.smoothstep(inner, 1.0, radial);
        let ring = b.multiply(limbal_darkness, ring);
        let ring = kit::one_minus(b, ring);
        let iris = b.multiply(iris, ring);
        let albedo = b.lerp(&sclera, iris, &mask);
        let forward = b.out(refraction, "forward");
        let bulged = b.out(refraction, "cornea");
        b.set(eye, "color", albedo)
            .set(eye, "iris", mask)
            .set(eye, "forward", forward)
            .set(eye, "normal", bulged)
            .set(eye, "caustic", caustic);
    });
    let shader = builder.out(eye, "shader");
    builder.output([("surface", shader.into()), ("alpha", alpha.into())]);
    builder.finish()
}
