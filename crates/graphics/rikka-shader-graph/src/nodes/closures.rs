use crate::library::{Behavior, ClosureCode, Field, Hosts, NodeSpec};
use crate::node::{DirectionSpace, Input, PortSpec};
use crate::ops::Category;
use crate::types::ValueType;

const SHADER: PortSpec = PortSpec::output("shader", ValueType::Shader);

const NORMAL: PortSpec = PortSpec::context(
    "normal",
    ValueType::Float3,
    Input::Normal(DirectionSpace::World),
);

fn closure(
    id: &'static str,
    title: &'static str,
    doc: &'static str,
    inputs: Vec<(PortSpec, Field)>,
    code: ClosureCode,
) -> NodeSpec {
    NodeSpec {
        id,
        title,
        category: Category::Shader,
        doc,
        hosts: Hosts::MATERIAL,
        inputs: inputs.iter().map(|(port, _)| *port).collect(),
        outputs: vec![SHADER],
        behavior: Behavior::Closure(ClosureCode {
            fields: inputs
                .iter()
                .map(|(port, field)| (port.name, *field))
                .collect(),
            ..code
        }),
    }
}

fn unit(name: &'static str, value: f32) -> PortSpec {
    PortSpec::number(name, value).range(0.0, 1.0)
}

fn common() -> Vec<(PortSpec, Field)> {
    vec![
        (unit("alpha", 1.0), Field::Alpha),
        (NORMAL, Field::Normal),
        (unit("occlusion", 1.0), Field::Occlusion),
        (PortSpec::color("emission", 0.0, 0.0, 0.0), Field::Emission),
    ]
}

fn principled() -> NodeSpec {
    let data = |port: PortSpec| (port, Field::Data(port.name));
    let mut inputs = vec![
        (PortSpec::color("base_color", 0.8, 0.8, 0.8), Field::Albedo),
        (unit("metalness", 0.0), Field::Data("metalness")),
        (unit("roughness", 0.3), Field::Data("specular_roughness")),
        data(unit("diffuse_roughness", 0.0)),
        data(unit("specular_weight", 1.0)),
        data(PortSpec::color("specular_color", 1.0, 1.0, 1.0)),
        (unit("anisotropy", 0.0), Field::Data("specular_anisotropy")),
        (
            PortSpec::number("ior", 1.5).soft(1.0, 3.0),
            Field::Data("specular_ior"),
        ),
        data(PortSpec::vector("tangent", 0.0, 0.0, 0.0)),
        (unit("second_lobe", 0.0), Field::Data("second_lobe")),
        (
            unit("second_roughness", 0.3),
            Field::Data("second_roughness"),
        ),
        data(unit("cavity", 1.0)),
        data(unit("transmission_weight", 0.0)),
        data(PortSpec::color("transmission_color", 1.0, 1.0, 1.0)),
        data(PortSpec::number("transmission_depth", 0.0).soft(0.0, 1.0)),
        data(PortSpec::number("dispersion", 0.0).soft(0.0, 1.0)),
        data(PortSpec::number("thickness", 0.0).soft(0.0, 0.1)),
        data(unit("subsurface_weight", 0.0)),
        data(PortSpec::color("subsurface_color", 0.8, 0.8, 0.8)),
        data(PortSpec::vector("subsurface_radius", 1.0, 0.5, 0.25)),
        data(PortSpec::number("subsurface_anisotropy", 0.0).range(-1.0, 1.0)),
        data(unit("coat_weight", 0.0)),
        data(PortSpec::color("coat_color", 1.0, 1.0, 1.0)),
        data(unit("coat_roughness", 0.0)),
        data(unit("coat_anisotropy", 0.0)),
        data(PortSpec::number("coat_ior", 1.6).soft(1.0, 3.0)),
        data(unit("coat_darkening", 1.0)),
        data(PortSpec::vector("coat_normal", 0.0, 0.0, 0.0)),
        data(unit("fuzz_weight", 0.0)),
        data(PortSpec::color("fuzz_color", 1.0, 1.0, 1.0)),
        data(unit("fuzz_roughness", 0.5)),
        data(unit("thin_film_weight", 0.0)),
        data(PortSpec::number("thin_film_thickness", 0.5).soft(0.0, 2.0)),
        data(PortSpec::number("thin_film_ior", 1.4).soft(1.0, 3.0)),
        (
            PortSpec::toggle("thin_walled", false),
            Field::Flag("thin_walled"),
        ),
    ];
    inputs.extend(common());
    closure(
        "bsdf.principled",
        "Principled BSDF",
        "The OpenPBR surface: a metal or dielectric base under optional coat, fuzz and thin film, with subsurface scattering and transmission.",
        inputs,
        ClosureCode {
            model: "RikkaOpenPbrModel",
            data: "rikka_openpbr_data()",
            fields: Vec::new(),
            features: vec![
                ("coat_weight", 0),
                ("fuzz_weight", 1),
                ("thin_film_weight", 2),
                ("subsurface_weight", 3),
                ("transmission_weight", 4),
                ("anisotropy", 5),
                ("second_lobe", 6),
            ],
            subsurface: Some("subsurface_weight"),
            transmission: Some("transmission_weight"),
            unscattered: Some("unscattered"),
        },
    )
}

fn plain(model: &'static str, data: &'static str) -> ClosureCode {
    ClosureCode {
        model,
        data,
        fields: Vec::new(),
        features: Vec::new(),
        subsurface: None,
        transmission: None,
        unscattered: None,
    }
}

fn standard() -> NodeSpec {
    let mut inputs = vec![
        (PortSpec::color("base_color", 0.8, 0.8, 0.8), Field::Albedo),
        (unit("metalness", 0.0), Field::Data("metallic")),
        (unit("roughness", 0.5), Field::Data("roughness")),
    ];
    inputs.extend(common());
    closure(
        "bsdf.standard",
        "Standard BSDF",
        "A light metallic-roughness surface: diffuse plus one GGX highlight.",
        inputs,
        plain("RikkaStandardModel", "rikka_closure_standard_data()"),
    )
}

fn hair() -> NodeSpec {
    let data = |port: PortSpec| (port, Field::Data(port.name));
    let mut inputs = vec![
        (PortSpec::color("color", 0.3, 0.2, 0.12), Field::Albedo),
        (
            PortSpec::context(
                "tangent",
                ValueType::Float3,
                Input::Tangent(DirectionSpace::World),
            ),
            Field::Data("tangent"),
        ),
        data(unit("roughness", 0.35)),
        data(PortSpec::number("shift", 0.035).soft(-0.2, 0.2)),
        data(PortSpec::number("specular", 1.0).soft(0.0, 2.0)),
        data(PortSpec::number("backlit", 1.0).soft(0.0, 2.0)),
        data(unit("scatter", 0.4)),
    ];
    inputs.extend(common());
    closure(
        "bsdf.hair",
        "Hair BSDF",
        "Strands lit along their tangent: the primary and tinted secondary highlights and the glow of light passing through.",
        inputs,
        plain("RikkaHairModel", "rikka_closure_hair_data()"),
    )
}

fn eye() -> NodeSpec {
    let data = |port: PortSpec| (port, Field::Data(port.name));
    let mut inputs = vec![
        (PortSpec::color("color", 0.9, 0.9, 0.9), Field::Albedo),
        (
            PortSpec::context(
                "forward",
                ValueType::Float3,
                Input::Normal(DirectionSpace::World),
            ),
            Field::Data("forward"),
        ),
        data(unit("iris", 0.0)),
        data(unit("cornea_roughness", 0.04)),
        data(PortSpec::number("cornea_ior", 1.376).soft(1.0, 2.0)),
        data(PortSpec::number("caustic", 2.0).soft(0.0, 8.0)),
        data(unit("sclera_scatter", 0.0)),
    ];
    inputs.extend(common());
    closure(
        "bsdf.eye",
        "Eye BSDF",
        "An eyeball under a clear cornea: iris lit through the refracting cornea with its caustic, sclera softened by scatter.",
        inputs,
        plain("RikkaEyeModel", "rikka_closure_eye_data()"),
    )
}

fn diffuse() -> NodeSpec {
    let mut inputs = vec![
        (PortSpec::color("color", 0.8, 0.8, 0.8), Field::Albedo),
        (unit("roughness", 0.0), Field::Data("roughness")),
    ];
    inputs.extend(common());
    closure(
        "bsdf.diffuse",
        "Diffuse BSDF",
        "Matte reflection: Lambert at roughness 0, the energy-preserving Oren-Nayar (EON) model above it.",
        inputs,
        plain("RikkaDiffuseModel", "rikka_closure_diffuse_data()"),
    )
}

fn glossy() -> NodeSpec {
    let mut inputs = vec![
        (PortSpec::color("color", 0.8, 0.8, 0.8), Field::Albedo),
        (unit("roughness", 0.3), Field::Data("roughness")),
        (unit("anisotropy", 0.0), Field::Data("anisotropy")),
        (
            PortSpec::context(
                "tangent",
                ValueType::Float3,
                Input::Tangent(DirectionSpace::World),
            ),
            Field::Data("tangent"),
        ),
    ];
    inputs.extend(common());
    closure(
        "bsdf.glossy",
        "Glossy BSDF",
        "Mirror-like GGX reflection tinted by color at every angle, stretched along the tangent by anisotropy, with multiple scattering restored.",
        inputs,
        plain("RikkaGlossyModel", "rikka_closure_glossy_data()"),
    )
}

fn sheen() -> NodeSpec {
    let mut inputs = vec![
        (PortSpec::color("color", 1.0, 1.0, 1.0), Field::Albedo),
        (unit("roughness", 0.5), Field::Data("roughness")),
    ];
    inputs.extend(common());
    closure(
        "bsdf.sheen",
        "Sheen BSDF",
        "The soft grazing glow of velvet and fuzz (Zeltner's microfibre sheen, OpenPBR fuzz); nothing shows beneath it, so add it over a base.",
        inputs,
        plain("RikkaSheenModel", "rikka_closure_sheen_data()"),
    )
}

fn translucent() -> NodeSpec {
    let mut inputs = vec![(PortSpec::color("color", 0.8, 0.8, 0.8), Field::Albedo)];
    inputs.extend(common());
    closure(
        "bsdf.translucent",
        "Translucent BSDF",
        "Light passing diffusely through a thin sheet: lit from behind the normal, Lambert-shaped.",
        inputs,
        plain("RikkaTranslucentModel", "rikka_closure_translucent_data()"),
    )
}

fn toon() -> NodeSpec {
    let mut inputs = vec![
        (PortSpec::color("color", 0.8, 0.8, 0.8), Field::Albedo),
        (unit("size", 0.5), Field::Data("size")),
        (unit("smooth", 0.0), Field::Data("smoothness")),
        (
            PortSpec::options("component", &["diffuse", "glossy"], 0),
            Field::Data("component"),
        ),
    ];
    inputs.extend(common());
    closure(
        "bsdf.toon",
        "Toon BSDF",
        "Blender's toon BSDF: full light within size × 90° of the normal (diffuse) or of the mirror direction (glossy), fading over smooth × 90° more (widened by the light's angular size), normalized as in Cycles.",
        inputs,
        plain("RikkaToonBsdfModel", "rikka_closure_toon_data()"),
    )
}

fn skin() -> NodeSpec {
    let data = |port: PortSpec| (port, Field::Data(port.name));
    let mut inputs = vec![
        (PortSpec::color("color", 0.8, 0.6, 0.5), Field::Albedo),
        data(unit("subsurface_weight", 1.0)),
        data(PortSpec::color("subsurface_color", 0.8, 0.6, 0.5)),
        data(PortSpec::vector(
            "subsurface_radius",
            0.004,
            0.00148,
            0.00076,
        )),
        data(PortSpec::number("subsurface_anisotropy", 0.0).range(-1.0, 1.0)),
        data(PortSpec::number("thickness", 0.0).soft(0.0, 0.05)),
        (unit("roughness", 0.5), Field::Data("specular_roughness")),
        data(unit("second_roughness", 0.3)),
        data(unit("second_lobe", 0.15)),
        (
            PortSpec::number("ior", 1.4).soft(1.0, 2.0),
            Field::Data("specular_ior"),
        ),
        data(unit("specular_weight", 1.0)),
        data(unit("diffuse_roughness", 0.0)),
        data(unit("cavity", 1.0)),
        data(unit("fuzz_weight", 0.0)),
        data(PortSpec::color("fuzz_color", 1.0, 1.0, 1.0)),
        data(unit("fuzz_roughness", 0.6)),
    ];
    inputs.extend(common());
    closure(
        "bsdf.skin",
        "Skin BSDF",
        "Skin over the OpenPBR model: screen-space subsurface scattering with radii in metres, a dual specular lobe, cavity and optional fuzz; subsurface_color is usually the albedo.",
        inputs,
        ClosureCode {
            model: "RikkaOpenPbrModel",
            data: "rikka_closure_skin_data()",
            fields: Vec::new(),
            features: vec![
                ("fuzz_weight", 1),
                ("subsurface_weight", 3),
                ("second_lobe", 6),
            ],
            subsurface: Some("subsurface_weight"),
            transmission: None,
            unscattered: Some("unscattered"),
        },
    )
}

pub(crate) fn specs() -> Vec<NodeSpec> {
    vec![
        principled(),
        standard(),
        hair(),
        eye(),
        diffuse(),
        glossy(),
        sheen(),
        translucent(),
        toon(),
        skin(),
    ]
}
