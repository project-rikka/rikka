use rikka_shader_graph::{
    Arg, Builder, Constant, Error, Graph, GraphKind, Input, Library, NodeKind, Op, ParameterDecl,
    Wire,
};
use rikka_slang::{Hint, Literal, ParameterKind, TextureKind};

use crate::import::{self, Class, classed};

const MMD_LIGHT: f32 = 154.0 / 255.0;
const MTOON_STEP: f32 = 2.0;
const SCREEN_WIDTH: f32 = 3.0;
const MATCAP_VIEW_NORMAL: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Style {
    pub mmd: bool,
    pub mtoon: bool,
    pub anime: bool,
}

impl Style {
    pub(crate) const MMD: Self = Self {
        mmd: true,
        mtoon: false,
        anime: false,
    };

    pub(crate) const MTOON: Self = Self {
        mmd: false,
        mtoon: true,
        anime: false,
    };

    pub(crate) const ANIME: Self = Self {
        mmd: true,
        mtoon: true,
        anime: true,
    };
}

struct Draft<'b, 'l> {
    builder: &'b mut Builder<'l>,
}

impl Draft<'_, '_> {
    fn number(&mut self, value: f32) -> Wire {
        self.builder.constant(Constant::scalar(value))
    }

    fn port<const N: usize>(&mut self, id: &str, inputs: [(&str, Arg); N], port: &str) -> Wire {
        let node = self.builder.external(id, inputs);
        self.builder.out(node, port)
    }

    fn sample(&mut self, kind: TextureKind, texture: &Wire, uv: &Wire, port: &str) -> Wire {
        let node = self
            .builder
            .texture(kind, [("texture", texture.into()), ("uv", uv.into())]);
        self.builder.out(node, port)
    }

    fn multiply(&mut self, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
        self.builder.multiply(a, b)
    }

    fn add(&mut self, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
        self.builder.add(a, b)
    }

    fn plus(&mut self, a: Wire, b: Option<&Wire>) -> Wire {
        match b {
            Some(b) => self.builder.add(a, b),
            None => a,
        }
    }

    fn one_minus(&mut self, value: &Wire) -> Wire {
        self.builder.op(Op::OneMinus, [value.into()])
    }

    fn parameter(&mut self, decl: ParameterDecl) -> Wire {
        self.builder.parameter(decl)
    }

    fn float(&mut self, name: &str, default: f32, group: &str) -> Wire {
        self.parameter(ParameterDecl::float(name, default).with_group(group))
    }

    fn unit(&mut self, name: &str, default: f32, group: &str) -> Wire {
        self.parameter(
            ParameterDecl::float(name, default)
                .with_range(0.0, 1.0)
                .with_group(group),
        )
    }

    fn color(&mut self, name: &str, default: [f32; 3], group: &str) -> Wire {
        self.parameter(
            ParameterDecl::new(
                name,
                ParameterKind::Color {
                    alpha: false,
                    srgb: true,
                },
            )
            .with_default(Literal::Vec3(default))
            .with_group(group),
        )
    }

    fn plane(&mut self, name: &str, default: [f32; 2], group: &str) -> Wire {
        self.parameter(
            ParameterDecl::new(name, ParameterKind::Vec2)
                .with_default(Literal::Vec2(default))
                .with_group(group),
        )
    }

    fn texture(&mut self, name: &str, kind: TextureKind, group: &str) -> Wire {
        self.parameter(ParameterDecl::texture(name, kind).with_group(group))
    }

    fn feature(&mut self, name: &str, presets: &[(&[Class], f32)]) -> Wire {
        let mut decl = ParameterDecl::float(name, 0.0)
            .with_range(0.0, 1.0)
            .with_group("anime");
        decl.presets = classed(presets);
        self.parameter(decl)
    }
}

struct Rim {
    color: Wire,
    mix: Wire,
}

pub(crate) fn character(library: &Library, style: Style) -> Result<Graph, Error> {
    let mut builder = Builder::new(GraphKind::Material, library);
    let mut draft = Draft {
        builder: &mut builder,
    };
    shade(&mut draft, style);
    builder.finish()
}

fn animated_uv(d: &mut Draft<'_, '_>) -> Wire {
    let plain = d.builder.input(Input::Uv);
    let mask_texture = d.texture("uv_animation_mask_texture", TextureKind::Data, "uv");
    let mask = d.sample(TextureKind::Data, &mask_texture, &plain, "rgba");
    let mask = d.builder.swizzle(mask, "z");
    let offset = d.plane("uv_offset", [0.0, 0.0], "uv");
    let scale = d.plane("uv_scale", [1.0, 1.0], "uv");
    let rotation = d.parameter(
        ParameterDecl::float("uv_rotation", 0.0)
            .with_group("uv")
            .with_hint(Hint::Angle),
    );
    let scroll = d.plane("uv_scroll", [0.0, 0.0], "uv");
    let spin = d.float("uv_spin", 0.0, "uv");
    d.port(
        "vector.uv_transform",
        [
            ("uv", plain.into()),
            ("offset", offset.into()),
            ("scale", scale.into()),
            ("rotation", rotation.into()),
            ("scroll", scroll.into()),
            ("spin", spin.into()),
            ("mask", mask.into()),
        ],
        "uv",
    )
}

fn mtoon_rim(d: &mut Draft<'_, '_>, normal: &Wire, uv: &Wire) -> Rim {
    let matcap_texture = d.texture("matcap_texture", TextureKind::Colour, "rim");
    let matcap_color = d.color("matcap_color", [1.0, 1.0, 1.0], "rim");
    let at = d.port("npr.matcap_uv", [("normal", normal.into())], "uv");
    let read = d.sample(TextureKind::Colour, &matcap_texture, &at, "rgb");
    let bound = d.port(
        "texture.bound",
        [("texture", matcap_texture.into())],
        "value",
    );
    let matcap = d.multiply(read, matcap_color);
    let matcap = d.multiply(matcap, bound);
    let rim_color = d.color("rim_color", [0.0, 0.0, 0.0], "rim");
    let power = d.parameter(
        ParameterDecl::float("rim_fresnel_power", 5.0)
            .with_soft_range(0.0, 100.0)
            .with_group("rim"),
    );
    let lift = d.unit("rim_lift", 0.0, "rim");
    let fresnel = d.port(
        "npr.fresnel_rim",
        [
            ("normal", normal.into()),
            ("power", power.into()),
            ("lift", lift.into()),
        ],
        "value",
    );
    let parametric = d.multiply(rim_color, fresnel);
    let rim = d.add(matcap, parametric);
    let rim_texture = d.texture("rim_texture", TextureKind::Colour, "rim");
    let painted = d.sample(TextureKind::Colour, &rim_texture, uv, "rgb");
    let color = d.multiply(rim, painted);
    let mix = d.unit("rim_lighting_mix", 1.0, "rim");
    Rim { color, mix }
}

fn shade(d: &mut Draft<'_, '_>, style: Style) {
    let uv = match style.mtoon {
        true => animated_uv(d),
        false => d.builder.input(Input::Uv),
    };

    let base_color = import::base_color(d.builder);
    let base_texture = import::base_texture(d.builder);
    let picture = d.builder.texture(
        TextureKind::Colour,
        [("texture", base_texture.into()), ("uv", (&uv).into())],
    );
    let picture_rgb = d.builder.out(picture, "rgb");
    let picture_alpha = d.builder.out(picture, "a");
    let vertex = d.builder.input(Input::VertexColor);
    let vertex = d.builder.swizzle(vertex, "xyz");
    let color = d.builder.swizzle(&base_color, "xyz");
    let material = match style.mmd {
        true => {
            let ambient = import::ambient_color(d.builder);
            let lifted = d.add(color, ambient);
            d.builder.saturate(lifted)
        }
        false => color,
    };
    let painted = d.multiply(&material, &picture_rgb);
    let mut albedo = d.multiply(&painted, &vertex);

    let reject = import::skin_reject(d.builder);
    let reject = d.port(
        "npr.skin_reject",
        [("color", painted.into()), ("amount", reject.into())],
        "value",
    );
    let opacity = d.builder.swizzle(&base_color, "w");
    let alpha = d.multiply(opacity, picture_alpha);
    let alpha = d.multiply(alpha, reject);

    let normal_texture = import::normal_texture(d.builder);
    let normal_scale = import::normal_scale(d.builder);
    let tangent_normal = d.sample(TextureKind::Normal, &normal_texture, &uv, "normal");
    let normal = d
        .builder
        .op(Op::NormalMap, [tangent_normal.into(), normal_scale.into()]);

    let shade_color = import::shade_color(d.builder);
    let shade_picture = match style.mtoon {
        true => {
            let shade_texture = d.texture("shade_texture", TextureKind::Colour, "shade");
            d.sample(TextureKind::Colour, &shade_texture, &uv, "rgb")
        }
        false => picture_rgb.clone(),
    };
    let shade_painted = d.multiply(shade_color, shade_picture);
    let mut shade = d.multiply(shade_painted, &vertex);

    let mut added = None;
    if style.mmd {
        let sphere_texture = import::sphere_texture(d.builder);
        let mode = import::sphere_mode(d.builder);
        let flipped = d.number(1.0);
        let view_normal = d.number(MATCAP_VIEW_NORMAL);
        let at = d.port(
            "npr.matcap_uv",
            [
                ("normal", (&normal).into()),
                ("mode", view_normal.into()),
                ("flip", flipped.into()),
            ],
            "uv",
        );
        let sphere = d.sample(TextureKind::Colour, &sphere_texture, &at, "rgb");
        let compare = |d: &mut Draft<'_, '_>, index: f32| {
            let target = d.number(index);
            let close = d.number(0.25);
            let equal = d.number(4.0);
            d.builder.op(
                Op::Compare,
                [(&mode).into(), target.into(), close.into(), equal.into()],
            )
        };
        let adding = compare(d, import::SphereMode::Add.index() as f32);
        let present = d.port(
            "texture.bound",
            [("texture", sphere_texture.into())],
            "value",
        );
        let adding = d.multiply(adding, present);
        let multiplying = compare(d, import::SphereMode::Multiply.index() as f32);
        let white = d.builder.constant(Constant::vec3(1.0, 1.0, 1.0));
        let factor = d.builder.lerp(white, &sphere, multiplying);
        albedo = d.multiply(albedo, &factor);
        shade = d.multiply(shade, &factor);
        added = Some(d.multiply(sphere, adding));
    }

    let rim = style.mtoon.then(|| mtoon_rim(d, &normal, &uv));
    let rim_lit = rim.as_ref().map(|rim| d.multiply(&rim.color, &rim.mix));
    let extra = match (&added, &rim_lit) {
        (Some(added), Some(rim)) => Some(d.add(added, rim)),
        (Some(added), None) => Some(added.clone()),
        (None, Some(rim)) => Some(rim.clone()),
        (None, None) => None,
    };

    let porosity = import::porosity(d.builder);
    let wet = d.builder.external(
        "npr.wet",
        [
            ("albedo", albedo.into()),
            ("porosity", (&porosity).into()),
            ("normal", (&normal).into()),
        ],
    );
    let lit = d.builder.out(wet, "albedo");
    let film = d.builder.out(wet, "film");
    let film_normal = d.builder.out(wet, "film_normal");
    let film_view = d.builder.out(wet, "film_view");
    let dry = d.one_minus(&film_view);
    let shade = d.port(
        "npr.wet",
        [
            ("albedo", shade.into()),
            ("porosity", porosity.into()),
            ("normal", (&normal).into()),
        ],
        "albedo",
    );

    let light = d.builder.input(Input::LightDirection);
    let sample = d
        .builder
        .external("light.sample", [("normal", (&normal).into())]);
    let radiance = d.builder.out(sample, "color");
    let attenuation = d.builder.out(sample, "attenuation");
    let shadowing = d.builder.external("light.shadow", []);
    let mut shadow = d.builder.out(shadowing, "map");
    let tint = d.builder.out(shadowing, "filter");
    let scale = d.port("npr.light_scale", [], "value");
    let falloff = d.multiply(attenuation, scale);
    let tinted = d.multiply(radiance, tint);
    let irradiance = d.multiply(tinted, falloff);
    let mut facing = d.builder.dot(&normal, &light);

    let anime = style.anime.then(|| {
        let hair_shadow = d.feature(
            "hair_shadow",
            &[(
                &[
                    Class::Skin,
                    Class::Face,
                    Class::Mouth,
                    Class::Lash,
                    Class::Blush,
                    Class::Eye,
                ],
                1.0,
            )],
        );
        let face_shadow = d.feature(
            "face_shadow",
            &[(&[Class::Face, Class::Mouth, Class::Blush], 1.0)],
        );
        let scatter = d.feature(
            "scatter",
            &[(
                &[
                    Class::Skin,
                    Class::Face,
                    Class::Mouth,
                    Class::Lash,
                    Class::Blush,
                ],
                0.25,
            )],
        );
        let strands = d.feature("strands", &[(&[Class::Hair], 1.0)]);
        let depth_rim = d.feature(
            "depth_rim",
            &[(
                &[
                    Class::Cloth,
                    Class::Skin,
                    Class::Face,
                    Class::Hair,
                    Class::Stocking,
                ],
                0.6,
            )],
        );
        let distance = d.parameter(
            ParameterDecl::float("hair_shadow_distance", 0.03)
                .with_soft_range(0.0, 0.2)
                .with_group("anime"),
        );
        let reach = d.parameter(
            ParameterDecl::float("hair_shadow_reach", 0.15)
                .with_soft_range(0.0, 1.0)
                .with_group("anime"),
        );
        let hair = d.port(
            "npr.hair_shadow",
            [
                ("light", (&light).into()),
                ("distance", distance.into()),
                ("reach", reach.into()),
            ],
            "lit",
        );
        let one = d.number(1.0);
        let hair = d.builder.lerp(one, hair, hair_shadow);
        shadow = d.multiply(&shadow, hair);
        let face = d.port("npr.face_procedural", [("light", (&light).into())], "value");
        facing = d.builder.lerp(&facing, face, face_shadow);
        (scatter, strands, depth_rim)
    });

    let mut shifted = facing.clone();
    if style.mtoon {
        let shift = d.parameter(
            ParameterDecl::float("shading_shift", 0.0)
                .with_soft_range(-1.0, 1.0)
                .with_group("shade"),
        );
        let shift_texture = d.texture("shading_shift_texture", TextureKind::Data, "shade");
        let shift_scale = d.float("shading_shift_scale", 1.0, "shade");
        let painted = d.sample(TextureKind::Data, &shift_texture, &uv, "rgba");
        let painted = d.builder.swizzle(painted, "x");
        let bound = d.port(
            "texture.bound",
            [("texture", shift_texture.into())],
            "value",
        );
        let painted = d.multiply(painted, shift_scale);
        let painted = d.multiply(painted, bound);
        let shift = d.add(shift, painted);
        shifted = d.add(shifted, shift);
    }
    let toony = d.unit(
        "shading_toony",
        match style.mmd {
            true => 0.95,
            false => 0.9,
        },
        "shade",
    );
    let edge = d.number(0.0);
    let mode = d.number(MTOON_STEP);
    let step = d.port(
        "npr.toon_step",
        [
            ("value", shifted.into()),
            ("edge", edge.into()),
            ("mode", mode.into()),
            ("toony", toony.into()),
        ],
        "value",
    );
    let shading = d.multiply(step, &shadow);
    let full = d.plus(lit.clone(), extra.as_ref());
    let cel = d.builder.lerp(&shade, &lit, &shading);
    let unramped = match &extra {
        Some(extra) => {
            let lifted = d.multiply(extra, &shading);
            d.add(cel, lifted)
        }
        None => cel,
    };
    let toned = match style.mmd {
        true => {
            let toon_texture = import::toon_texture(d.builder);
            let bound = d.port(
                "texture.bound",
                [("texture", (&toon_texture).into())],
                "value",
            );
            let ramp = d.port(
                "npr.vertical_ramp",
                [
                    ("texture", (&toon_texture).into()),
                    ("facing", (&facing).into()),
                ],
                "color",
            );
            let lowest = d.number(-1.0);
            let bottom = d.port(
                "npr.vertical_ramp",
                [("texture", toon_texture.into()), ("facing", lowest.into())],
                "color",
            );
            let tone = d.builder.lerp(bottom, ramp, &shadow);
            let ramped = d.multiply(&full, tone);
            d.builder
                .op(Op::Switch, [unramped.into(), ramped.into(), bound.into()])
        }
        false => unramped,
    };
    let mut direct = d.multiply(&irradiance, toned);

    if style.mmd {
        let specular_color = import::specular_color(d.builder);
        let power = import::specular_power(d.builder);
        let term = d.port(
            "npr.stepped_specular",
            [
                ("normal", (&normal).into()),
                ("light", (&light).into()),
                ("shininess", power.into()),
            ],
            "term",
        );
        let level = d.number(MMD_LIGHT);
        let reflect = d.multiply(specular_color, level);
        let highlight = d.multiply(reflect, term);
        let highlight = d.multiply(highlight, &shadow);
        let highlight = d.multiply(highlight, &irradiance);
        direct = d.add(direct, highlight);
    }

    if let Some((scatter, strands, depth_rim)) = anime {
        let bands = d.port("npr.scatter_bands", [("facing", (&facing).into())], "value");
        let bands = d.multiply(bands, scatter);
        let warmed = d.multiply(bands, &lit);
        let warmed = d.multiply(warmed, &irradiance);
        direct = d.add(direct, warmed);

        let glossed = d.port(
            "npr.kajiya_kay",
            [("normal", (&normal).into()), ("light", (&light).into())],
            "color",
        );
        let glossed = d.multiply(glossed, strands);
        let glossed = d.multiply(glossed, &lit);
        let glossed = d.multiply(glossed, &shadow);
        let glossed = d.multiply(glossed, &irradiance);
        direct = d.add(direct, glossed);

        let mask = d.port("npr.depth_rim", [("normal", (&normal).into())], "mask");
        let side = d.port("npr.light_side", [("facing", (&facing).into())], "value");
        let edge = d.multiply(mask, side);
        let edge = d.multiply(edge, depth_rim);
        let edge = d.multiply(edge, &lit);
        let edge = d.multiply(edge, &irradiance);
        direct = d.add(direct, edge);
    }

    let glint = d.port(
        "npr.wet_glint",
        [
            ("film_normal", (&film_normal).into()),
            ("film", (&film).into()),
            ("light", (&light).into()),
        ],
        "value",
    );
    let glint = d.multiply(glint, &shadow);
    let glint = d.multiply(glint, &irradiance);
    let direct = d.multiply(direct, &dry);
    let direct = d.add(direct, glint);

    let sky = match style.mtoon {
        true => {
            let equalization = d.unit("gi_equalization", 0.9, "shade");
            d.port(
                "light.environment",
                [
                    ("normal", (&normal).into()),
                    ("equalization", equalization.into()),
                ],
                "equalized",
            )
        }
        false => d.port("light.indirect", [("normal", (&normal).into())], "sky"),
    };
    let mut ambient = d.multiply(&sky, &full);
    if let Some(rim) = &rim {
        let unlit = d.one_minus(&rim.mix);
        let constant = d.multiply(&rim.color, unlit);
        ambient = d.add(ambient, constant);
    }
    let film_light = d.builder.external(
        "npr.wet_film",
        [("film_normal", film_normal.into()), ("film", film.into())],
    );
    let sheen = d.builder.out(film_light, "sheen");
    let reflection = d.builder.out(film_light, "reflection");
    let ambient = d.multiply(ambient, &dry);
    let sheen = d.multiply(sheen, &sky);
    let reflection = d.multiply(reflection, &film_view);
    let ambient = d.add(ambient, sheen);
    let ambient = d.add(ambient, reflection);

    let lighting = d.builder.shader(
        NodeKind::Lighting,
        [("light", (&direct).into()), ("ambient", (&ambient).into())],
    );
    let emission_color = import::emission_color(d.builder);
    let emission_texture = import::emission_texture(d.builder);
    let glow = d.sample(TextureKind::Colour, &emission_texture, &uv, "rgb");
    let glow = d.multiply(emission_color, glow);
    let glow = d
        .builder
        .shader(NodeKind::Emission, [("color", glow.into())]);
    let surface = d.builder.shader(
        NodeKind::AddShader,
        [("a", lighting.into()), ("b", glow.into())],
    );

    let outline_color = import::outline_color(d.builder);
    let outline = match style.mtoon {
        true => {
            let mix = d.unit("outline_lighting_mix", 1.0, "outline");
            let tinted = d.multiply(&outline_color, &mix);
            let light = d.multiply(&tinted, &direct);
            let shaded = d.multiply(&tinted, &ambient);
            let unlit = d.one_minus(&mix);
            let flat = d.multiply(&outline_color, unlit);
            let ambient = d.add(shaded, flat);
            d.builder.shader(
                NodeKind::Lighting,
                [("light", light.into()), ("ambient", ambient.into())],
            )
        }
        false => d
            .builder
            .shader(NodeKind::Emission, [("color", outline_color.into())]),
    };
    let mut width = import::outline_width(d.builder);
    let mode = match style.mtoon {
        true => {
            let width_texture = d.texture("outline_width_texture", TextureKind::Data, "outline");
            let painted = d.sample(TextureKind::Data, &width_texture, &uv, "rgba");
            let painted = d.builder.swizzle(painted, "y");
            width = d.multiply(width, painted);
            let mode = d.parameter(
                ParameterDecl::new(
                    "outline_width_mode",
                    ParameterKind::Enum {
                        options: vec!["world".to_owned(), "screen".to_owned()],
                    },
                )
                .with_default(Literal::Enum(0))
                .with_group("outline"),
            );
            let screen = d.number(SCREEN_WIDTH);
            d.multiply(mode, screen)
        }
        false => d.number(SCREEN_WIDTH),
    };
    let width = d.port(
        "npr.outline_width",
        [("width", width.into()), ("mode", mode.into())],
        "value",
    );
    d.builder.output([
        ("surface", surface.into()),
        ("alpha", alpha.into()),
        ("outline", outline.into()),
        ("outline_width", width.into()),
    ]);
}
