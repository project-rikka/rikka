use std::f32::consts::PI;

use rikka_shader_graph::{Builder, Error, Graph, GraphKind, Library, Op, Wire};
use rikka_slang::TextureKind;

use super::blocks;
use super::kit;
use super::realistic::{self, Lobes};
use crate::{Family, Preset};

pub(super) fn presets() -> Vec<Preset> {
    vec![Preset {
        key: "hybrid",
        title: "PBR-toon hybrid",
        summary: "Physically based at stylization 0, cel at 1: one slider turns diffuse into two tones, highlights into crisp shapes and ambient flat, each by its own share, and fades in an outline; a shadow ramp and a light-side depth rim (Wuthering Waves style) are optional.",
        family: Family::Stylized,
        build: hybrid,
    }]
}

struct Shares {
    diffuse: Wire,
    specular: Wire,
    ambient: Wire,
    blend: Wire,
}

fn shares(builder: &mut Builder<'_>) -> Shares {
    builder.frame("Stylization", |b| {
        let master = b.parameter(
            kit::unit("stylization", 0.5)
                .with_group("stylization")
                .with_label("Stylization"),
        );
        let share = |b: &mut Builder<'_>, name: &str, label: &str| {
            let weight = b.parameter(
                kit::unit(name, 1.0)
                    .with_group("stylization")
                    .with_label(label),
            );
            b.multiply(&master, weight)
        };
        let diffuse = share(b, "stylize_diffuse", "Diffuse share");
        let specular = share(b, "stylize_specular", "Specular share");
        let ambient = share(b, "stylize_ambient", "Ambient share");
        let either = kit::maximum(b, &specular, &ambient);
        let blend = kit::maximum(b, &diffuse, either);
        Shares {
            diffuse,
            specular,
            ambient,
            blend,
        }
    })
}

struct Shape {
    shade: Wire,
    edge: Wire,
    softness: Wire,
    highlight_edge: Wire,
    highlight_softness: Wire,
    ramp: Wire,
    ramp_share: Wire,
}

fn shape(builder: &mut Builder<'_>) -> Shape {
    builder.frame("Cel shape", |b| Shape {
        shade: b.parameter(super::shade_color([0.55, 0.55, 0.65])),
        edge: b.parameter(
            kit::unit("band_edge", 0.5)
                .with_group("cel")
                .with_label("Shadow edge"),
        ),
        softness: b.parameter(
            kit::amount("band_softness", 0.04, 0.0, 0.5)
                .with_group("cel")
                .with_label("Edge softness"),
        ),
        highlight_edge: b.parameter(
            kit::unit("highlight_edge", 0.5)
                .with_group("cel")
                .with_label("Highlight edge"),
        ),
        highlight_softness: b.parameter(
            kit::amount("highlight_softness", 0.05, 0.0, 0.5)
                .with_group("cel")
                .with_label("Highlight softness"),
        ),
        ramp: b.parameter(
            kit::texture("ramp_texture", TextureKind::Ramp)
                .with_group("cel")
                .with_label("Shadow ramp"),
        ),
        ramp_share: b.parameter(
            kit::unit("ramp_strength", 0.0)
                .with_group("cel")
                .with_label("Ramp share"),
        ),
    })
}

fn hybrid(library: &Library) -> Result<Graph, Error> {
    let mut b = Builder::new(GraphKind::Material, library);
    blocks::define_cel_band(&mut b);
    blocks::define_light_rim(&mut b);
    let lobes = Lobes {
        anisotropy: true,
        coat: true,
        fuzz: true,
        ..Lobes::NONE
    };
    let physical = realistic::openpbr(&mut b, lobes, &realistic::PLAIN);
    let shares = shares(&mut b);
    let shape = shape(&mut b);
    let (diffuse_albedo, f0) = b.frame("Diffuse and reflectance", |b| {
        let dielectric = kit::one_minus(b, &physical.metalness);
        let diffuse = b.multiply(&physical.albedo, dielectric);
        let f0 = b.lerp([0.04, 0.04, 0.04], &physical.albedo, &physical.metalness);
        let f0 = b.multiply(f0, &physical.specular);
        (diffuse, f0)
    });
    let normal = &physical.normal;
    let direct = b.frame("Stylized light (per light)", |b| {
        let light = kit::node(b, "light.sample", [("normal", normal.into())]);
        let radiance = b.out(light, "color");
        let falloff = b.out(light, "attenuation");
        let irradiance = b.out(light, "irradiance");
        let visible = kit::value(b, "light.shadow", "visibility", []);
        let physical_diffuse = b.multiply(&irradiance, &visible);
        let wrapped = kit::value(b, "npr.wrap", "value", [("normal", normal.into())]);
        let response = b.multiply(wrapped, &visible);
        let ramp = kit::value(
            b,
            "npr.ramp",
            "rgb",
            [("texture", (&shape.ramp).into()), ("u", (&response).into())],
        );
        let shade = b.lerp(&shape.shade, ramp, &shape.ramp_share);
        let tone = blocks::cel_tone(
            b,
            [
                ("value", response.into()),
                ("edge", (&shape.edge).into()),
                ("softness", (&shape.softness).into()),
                ("shade", shade.into()),
            ],
        );
        let cel_diffuse = b.multiply(tone, &falloff);
        let diffuse = b.lerp(physical_diffuse, cel_diffuse, &shares.diffuse);
        let diffuse = b.multiply(diffuse, &diffuse_albedo);
        let diffuse = b.multiply(diffuse, 1.0 / PI);

        let reflected = kit::value(
            b,
            "npr.toon_ggx",
            "value",
            [
                ("normal", normal.into()),
                ("roughness", (&physical.roughness).into()),
                ("f0", (&f0).into()),
            ],
        );
        let physical_specular = b.multiply(reflected, 0.25);
        let physical_specular = b.multiply(physical_specular, &irradiance);
        let physical_specular = b.multiply(physical_specular, &visible);
        let lobe = kit::value(
            b,
            "npr.toon_ggx",
            "value",
            [
                ("normal", normal.into()),
                ("roughness", (&physical.roughness).into()),
                ("f0", [1.0, 1.0, 1.0].into()),
            ],
        );
        let lobe = kit::channel(b, &lobe, "x");
        let rough = b.op(
            Op::Clamp,
            [(&physical.roughness).into(), 0.02.into(), 1.0.into()],
        );
        let alpha = b.multiply(&rough, &rough);
        let alpha2 = b.multiply(&alpha, &alpha);
        let spread = b.multiply(alpha2, PI);
        let peak = b.op(Op::Reciprocal, [spread.clone().into()]);
        let normalized = b.multiply(lobe, spread);
        let half = b.multiply(&shape.highlight_softness, 0.5);
        let low = b.subtract(&shape.highlight_edge, &half);
        let high = b.add(&shape.highlight_edge, half);
        let crisp = b.smoothstep(low, high, normalized);
        let shadowed = kit::value(
            b,
            "npr.toon_step",
            "value",
            [
                ("value", visible.into()),
                ("feather", (&shape.softness).into()),
            ],
        );
        let cel_specular = b.multiply(&f0, crisp);
        let cel_specular = b.multiply(cel_specular, peak);
        let cel_specular = b.multiply(cel_specular, 0.25);
        let cel_specular = b.multiply(cel_specular, &falloff);
        let cel_specular = b.multiply(cel_specular, shadowed);
        let specular = b.lerp(physical_specular, cel_specular, &shares.specular);
        let lit = b.add(diffuse, specular);
        b.multiply(lit, radiance)
    });
    let rim = b.frame("Light-side depth rim", |b| {
        let width = b.parameter(
            kit::amount("rim_width", 3.0, 0.0, 12.0)
                .with_group("rim")
                .with_label("Width (pixels)"),
        );
        let color = b.parameter(
            kit::color("rim_color", [1.0, 1.0, 1.0])
                .with_group("rim")
                .with_label("Colour"),
        );
        let strength = b.parameter(
            kit::amount("rim_strength", 0.0, 0.0, 4.0)
                .with_group("rim")
                .with_label("Strength"),
        );
        let found = blocks::depth_rim(b, normal, width);
        blocks::light_rim(
            b,
            [
                ("rim", found.into()),
                ("normal", normal.into()),
                ("color", color.into()),
                ("strength", strength.into()),
            ],
        )
    });
    let direct = b.add(direct, rim);
    let ambient = b.frame("Stylized ambient", |b| {
        let indirect = kit::node(
            b,
            "light.indirect",
            [
                ("normal", normal.into()),
                ("roughness", (&physical.roughness).into()),
                ("albedo", (&diffuse_albedo).into()),
                ("occlusion", (&physical.occlusion).into()),
            ],
        );
        let scene = b.out(indirect, "diffuse");
        let reflection = b.out(indirect, "reflection");
        let even = kit::value(
            b,
            "light.environment",
            "equalized",
            [("normal", normal.into()), ("equalization", 1.0.into())],
        );
        let even = b.multiply(even, &physical.occlusion);
        let diffuse = b.lerp(scene, even, &shares.ambient);
        let diffuse = b.multiply(diffuse, &diffuse_albedo);
        let specular = b.multiply(reflection, &f0);
        b.add(diffuse, specular)
    });
    let styled = kit::lighting(&mut b, &direct, &ambient);
    let glow = kit::emission(&mut b, &physical.emission);
    let styled = kit::add_shader(&mut b, &styled, &glow);
    let surface = kit::mix_shader(&mut b, &physical.shader, &styled, &shares.blend);
    let line = super::outline(&mut b, &physical.albedo, 0.0015, [0.2, 0.17, 0.2]);
    let width = b.multiply(line.width, &shares.blend);
    b.output([
        ("surface", surface.into()),
        ("alpha", physical.alpha.into()),
        ("outline", line.shader.into()),
        ("outline_width", width.into()),
    ]);
    b.finish()
}
