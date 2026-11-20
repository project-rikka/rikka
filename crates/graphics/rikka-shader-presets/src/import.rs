use rikka_shader_graph::{Builder, ParameterDecl, Wire};
use rikka_slang::{Literal, ParameterKind, Preset, Role, TextureKind};

pub const BASE_COLOR: &str = "base_color";
pub const BASE_TEXTURE: &str = "base_texture";
pub const SHADE_COLOR: &str = "shade_color";
pub const EMISSION_COLOR: &str = "emission_color";
pub const EMISSION_TEXTURE: &str = "emission_texture";
pub const NORMAL_TEXTURE: &str = "normal_texture";
pub const NORMAL_SCALE: &str = "normal_scale";
pub const OUTLINE_WIDTH: &str = "outline_width";
pub const OUTLINE_COLOR: &str = "outline_color";
pub const TOON_TEXTURE: &str = "toon_texture";
pub const SPHERE_TEXTURE: &str = "sphere_texture";
pub const SPHERE_MODE: &str = "sphere_mode";
pub const SPECULAR_COLOR: &str = "specular_color";
pub const SPECULAR_POWER: &str = "specular_power";
pub const AMBIENT_COLOR: &str = "ambient_color";
pub const POROSITY: &str = "porosity";
pub const SKIN_REJECT: &str = "skin_reject";
pub const SEE_THROUGH_AMOUNT: &str = "see_through.amount";
pub const SEE_THROUGH_REACH: &str = "see_through.reach";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Cloth,
    Skin,
    Face,
    Hair,
    Eye,
    Mouth,
    Lash,
    Blush,
    Stocking,
}

impl Class {
    pub const ALL: [Self; 9] = [
        Self::Cloth,
        Self::Skin,
        Self::Face,
        Self::Hair,
        Self::Eye,
        Self::Mouth,
        Self::Lash,
        Self::Blush,
        Self::Stocking,
    ];

    pub fn preset(self) -> &'static str {
        match self {
            Self::Cloth => "cloth",
            Self::Skin => "skin",
            Self::Face => "face",
            Self::Hair => "hair",
            Self::Eye => "eye",
            Self::Mouth => "mouth",
            Self::Lash => "lash",
            Self::Blush => "blush",
            Self::Stocking => "stocking",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SphereMode {
    None,
    #[default]
    Add,
    Multiply,
}

impl SphereMode {
    pub const ALL: [Self; 3] = [Self::None, Self::Add, Self::Multiply];

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Add => "add",
            Self::Multiply => "multiply",
        }
    }

    pub fn index(self) -> u32 {
        self as u32
    }
}

fn color(name: &str, alpha: bool, default: Literal, group: &str) -> ParameterDecl {
    ParameterDecl::new(name, ParameterKind::Color { alpha, srgb: true })
        .with_default(default)
        .with_group(group)
}

fn texture(name: &str, kind: TextureKind, group: &str) -> ParameterDecl {
    ParameterDecl::texture(name, kind).with_group(group)
}

pub fn base_color(builder: &mut Builder<'_>) -> Wire {
    builder
        .parameter(color(BASE_COLOR, true, Literal::Vec4([1.0; 4]), "base").with_role(Role::Albedo))
}

pub fn base_texture(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        texture(BASE_TEXTURE, TextureKind::Colour, "base").with_role(Role::AlbedoTexture),
    )
}

pub fn shade_color(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(color(SHADE_COLOR, false, Literal::Vec3([1.0; 3]), "shade"))
}

pub fn emission_color(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        color(EMISSION_COLOR, false, Literal::Vec3([0.0; 3]), "emission")
            .with_soft_range(0.0, 16.0)
            .with_role(Role::Emission),
    )
}

pub fn emission_texture(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        texture(EMISSION_TEXTURE, TextureKind::Colour, "emission").with_role(Role::EmissionTexture),
    )
}

pub fn normal_texture(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(texture(NORMAL_TEXTURE, TextureKind::Normal, "normal"))
}

pub fn normal_scale(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        ParameterDecl::float(NORMAL_SCALE, 1.0)
            .with_soft_range(0.0, 2.0)
            .with_group("normal"),
    )
}

pub fn outline_width(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        ParameterDecl::float(OUTLINE_WIDTH, 0.0)
            .with_soft_range(0.0, 0.05)
            .with_group("outline"),
    )
}

pub fn outline_color(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(color(
        OUTLINE_COLOR,
        false,
        Literal::Vec3([0.0; 3]),
        "outline",
    ))
}

pub fn toon_texture(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(texture(TOON_TEXTURE, TextureKind::Ramp, "toon"))
}

pub fn sphere_texture(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(texture(SPHERE_TEXTURE, TextureKind::Colour, "sphere"))
}

pub fn sphere_mode(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        ParameterDecl::new(
            SPHERE_MODE,
            ParameterKind::Enum {
                options: SphereMode::ALL
                    .iter()
                    .map(|mode| mode.name().to_owned())
                    .collect(),
            },
        )
        .with_default(Literal::Enum(SphereMode::default().index()))
        .with_group("sphere"),
    )
}

pub fn specular_color(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(color(
        SPECULAR_COLOR,
        false,
        Literal::Vec3([0.0; 3]),
        "specular",
    ))
}

pub fn specular_power(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        ParameterDecl::float(SPECULAR_POWER, 5.0)
            .with_soft_range(0.0, 256.0)
            .with_group("specular"),
    )
}

pub fn ambient_color(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(color(AMBIENT_COLOR, false, Literal::Vec3([0.0; 3]), "base"))
}

pub fn porosity(builder: &mut Builder<'_>) -> Wire {
    let mut decl = ParameterDecl::float(POROSITY, 0.85)
        .with_range(0.0, 1.0)
        .with_group("wetness");
    decl.presets = classed(&[
        (&[Class::Cloth, Class::Stocking], 0.85),
        (&[Class::Hair], 0.6),
        (&[Class::Lash], 0.5),
        (&[Class::Eye, Class::Mouth], 0.0),
        (&[Class::Skin, Class::Face, Class::Blush], 0.3),
    ]);
    builder.parameter(decl)
}

pub fn skin_reject(builder: &mut Builder<'_>) -> Wire {
    builder.parameter(
        ParameterDecl::float(SKIN_REJECT, 1.0)
            .with_range(0.0, 1.0)
            .with_group("see through"),
    )
}

pub fn classed(values: &[(&[Class], f32)]) -> Vec<Preset> {
    values
        .iter()
        .flat_map(|(classes, value)| {
            classes.iter().map(|class| Preset {
                name: class.preset().to_owned(),
                value: Literal::Float(*value),
            })
        })
        .collect()
}
