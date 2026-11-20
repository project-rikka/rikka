use rikka_slang::TextureKind;
use serde::Serialize;

use crate::context::Site;
use crate::error::{At, GraphError};
use crate::flat::available;
use crate::graph::{Graph, GraphKind, Scope};
use crate::library::Library;
use crate::node::{BlendMode, Curve, Input, Layer, NodeKind, Noise, Ports, Ramp};
use crate::ops::{Category, Op};
use crate::ports;
use crate::types::Constant;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    pub kind: NodeKind,
    pub title: String,
    pub category: Category,
    pub doc: String,
    pub ports: Ports,
}

impl Graph {
    pub fn ports(
        &self,
        library: &Library,
        scope: &Scope,
        kind: &NodeKind,
    ) -> Result<Ports, GraphError> {
        ports::ports(self, library, scope, kind).map_err(|problem| {
            GraphError::new(
                At {
                    scope: scope.clone(),
                    node: None,
                    port: None,
                },
                problem,
            )
        })
    }

    pub fn catalog(&self, library: &Library, scope: &Scope) -> Vec<Entry> {
        let sites: &[Site] = match self.kind() {
            GraphKind::Material => &[
                Site::Vertex,
                Site::Hull,
                Site::Surface,
                Site::Light,
                Site::Indirect,
                Site::Post,
            ],
            GraphKind::Composite => &[Site::Pixel, Site::Uniform],
            GraphKind::World => &[Site::Uniform],
        };
        let mut kinds: Vec<NodeKind> = Op::ALL.into_iter().map(NodeKind::Op).collect();
        kinds.extend(
            [
                Constant::scalar(0.0),
                Constant::vec2(0.0, 0.0),
                Constant::vec3(0.0, 0.0, 0.0),
                Constant::vec4(0.0, 0.0, 0.0, 1.0),
            ]
            .map(NodeKind::Constant),
        );
        kinds.extend(
            self.parameters()
                .iter()
                .map(|parameter| NodeKind::Parameter(parameter.name.clone())),
        );
        kinds.extend(Input::ALL.map(NodeKind::Input));
        kinds.push(NodeKind::Swizzle("xyz".to_owned()));
        kinds.extend(
            [
                TextureKind::Colour,
                TextureKind::Normal,
                TextureKind::Data,
                TextureKind::Ramp,
            ]
            .map(NodeKind::Texture),
        );
        kinds.extend([Noise::Value, Noise::Gradient].map(NodeKind::Fbm));
        kinds.extend(BlendMode::ALL.map(NodeKind::Blend));
        kinds.push(NodeKind::ColorRamp(Ramp::default()));
        kinds.push(NodeKind::Curve(Curve::default()));
        kinds.extend([
            NodeKind::Lighting,
            NodeKind::Emission,
            NodeKind::Transparent,
            NodeKind::MixShader,
            NodeKind::AddShader,
            NodeKind::ShaderToRgb,
            NodeKind::Sample,
        ]);
        kinds.extend(Layer::ALL.map(NodeKind::Layer));
        kinds.extend(
            library
                .specs()
                .map(|spec| NodeKind::Extern(spec.id.to_owned())),
        );
        kinds.extend(
            self.functions()
                .keys()
                .filter(|name| *scope != Scope::Function((*name).clone()))
                .map(|name| NodeKind::Call(name.clone())),
        );
        if let Scope::Function(name) = scope
            && let Some(function) = self.function(name)
        {
            kinds.extend(
                function
                    .inputs
                    .iter()
                    .map(|port| NodeKind::FunctionInput(port.name.clone())),
            );
            kinds.extend(
                function
                    .outputs
                    .iter()
                    .map(|port| NodeKind::FunctionOutput(port.name.clone())),
            );
        }
        kinds.push(NodeKind::Output);
        kinds
            .into_iter()
            .filter(|kind| sites.iter().any(|site| available(kind, library, *site)))
            .filter_map(|kind| {
                let ports = ports::ports(self, library, scope, &kind).ok()?;
                Some(Entry {
                    title: kind.title(library),
                    category: kind.category(library),
                    doc: match &kind {
                        NodeKind::Call(name) => self
                            .function(name)
                            .filter(|function| !function.doc.is_empty())
                            .map_or_else(|| kind.doc(library), |function| function.doc.clone()),
                        _ => kind.doc(library),
                    },
                    ports,
                    kind,
                })
            })
            .collect()
    }
}

impl NodeKind {
    pub fn title(&self, library: &Library) -> String {
        match self {
            Self::Op(op) => op.info().title.to_owned(),
            Self::Constant(constant) => match constant.width() {
                1 => "Number".to_owned(),
                width => format!("Vector {width}"),
            },
            Self::Parameter(name) => name.clone(),
            Self::Input(input) => capitalized(&input.title()),
            Self::Swizzle(mask) => format!("Swizzle {mask}"),
            Self::Texture(kind) => match kind {
                TextureKind::Colour => "Colour texture",
                TextureKind::Data => "Data texture",
                TextureKind::Normal => "Normal texture",
                TextureKind::Ramp => "Ramp texture",
            }
            .to_owned(),
            Self::Fbm(noise) => match noise {
                Noise::Value => "Value fbm",
                Noise::Gradient => "Gradient fbm",
            }
            .to_owned(),
            Self::Blend(mode) => format!("Blend {}", blend_name(*mode)),
            Self::ColorRamp(_) => "Colour ramp".to_owned(),
            Self::Curve(_) => "Curve".to_owned(),
            Self::Lighting => "Custom lighting".to_owned(),
            Self::Emission => "Emission".to_owned(),
            Self::Transparent => "Transparent".to_owned(),
            Self::MixShader => "Mix shader".to_owned(),
            Self::AddShader => "Add shader".to_owned(),
            Self::ShaderToRgb => "Shader to colour".to_owned(),
            Self::Layer(layer) => format!("Render layer {}", layer.name()),
            Self::Sample => "Sample image".to_owned(),
            Self::Extern(id) => library
                .get(id)
                .map_or_else(|| id.clone(), |spec| spec.title.to_owned()),
            Self::Call(name) => name.clone(),
            Self::FunctionInput(name) => format!("Input {name}"),
            Self::FunctionOutput(name) => format!("Output {name}"),
            Self::Output => "Output".to_owned(),
        }
    }

    pub fn category(&self, library: &Library) -> Category {
        match self {
            Self::Op(op) => op.info().category,
            Self::Constant(_) => Category::Constant,
            Self::Parameter(_) => Category::Parameter,
            Self::Input(input) => input.category(),
            Self::Swizzle(_) => Category::Vector,
            Self::Texture(_) => Category::Texture,
            Self::Fbm(_) => Category::Noise,
            Self::Blend(_) => Category::Color,
            Self::ColorRamp(_) | Self::Curve(_) => Category::Ramp,
            Self::Lighting
            | Self::Emission
            | Self::Transparent
            | Self::MixShader
            | Self::AddShader
            | Self::ShaderToRgb => Category::Shader,
            Self::Layer(_) | Self::Sample => Category::Layer,
            Self::Extern(id) => library
                .get(id)
                .map_or(Category::Function, |spec| spec.category),
            Self::Call(_) | Self::FunctionInput(_) | Self::FunctionOutput(_) => Category::Function,
            Self::Output => Category::Output,
        }
    }

    pub fn doc(&self, library: &Library) -> String {
        match self {
            Self::Op(op) => op.info().doc.to_owned(),
            Self::Constant(_) => "A fixed value.".to_owned(),
            Self::Parameter(name) => {
                format!("The value of parameter {name}, set per material slot.")
            }
            Self::Input(input) => input.doc().to_owned(),
            Self::Swizzle(_) => {
                "Picks and reorders components by a mask of xyzw or rgba.".to_owned()
            }
            Self::Texture(kind) => match kind {
                TextureKind::Colour => "A colour texture sampled at uv.",
                TextureKind::Data => {
                    "A data texture (masks, roughness) sampled at uv, unconverted."
                }
                TextureKind::Normal => {
                    "A tangent-space normal map sampled at uv; feed it to Normal map."
                }
                TextureKind::Ramp => "A ramp texture read across its middle row at position.",
            }
            .to_owned(),
            Self::Fbm(noise) => format!(
                "Octaves of {} noise summed with falling weight, in 0…1.",
                match noise {
                    Noise::Value => "value",
                    Noise::Gradient => "gradient",
                }
            ),
            Self::Blend(mode) => format!(
                "base and blend combined like the {} layer mode, mixed in by opacity.",
                blend_name(*mode)
            ),
            Self::ColorRamp(_) => "Colour stops looked up by position.".to_owned(),
            Self::Curve(_) => "A value remapped through curve points.".to_owned(),
            Self::Lighting => {
                "Lighting you compute yourself: light is added once per light, ambient once from the surroundings."
                    .to_owned()
            }
            Self::Emission => "Light the surface gives off by itself.".to_owned(),
            Self::Transparent => "Lets what is behind show through.".to_owned(),
            Self::MixShader => "Blends two shaders by factor.".to_owned(),
            Self::AddShader => "Adds the light of two shaders.".to_owned(),
            Self::ShaderToRgb => {
                "The lit result of a shader as a colour, to restyle it with ramps and maths."
                    .to_owned()
            }
            Self::Layer(layer) => layer.doc().to_owned(),
            Self::Sample => "An image read at any uv, with filtering.".to_owned(),
            Self::Extern(id) => library
                .get(id)
                .map_or_else(String::new, |spec| spec.doc.to_owned()),
            Self::Call(name) => format!("Runs function {name}."),
            Self::FunctionInput(name) => format!("The value passed to function input {name}."),
            Self::FunctionOutput(name) => format!("What the function returns as {name}."),
            Self::Output => "What the graph produces.".to_owned(),
        }
    }
}

fn capitalized(text: &str) -> String {
    let mut letters = text.chars();
    letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default()
}

fn blend_name(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "normal",
        BlendMode::Multiply => "multiply",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::SoftLight => "soft light",
        BlendMode::Add => "add",
        BlendMode::Subtract => "subtract",
        BlendMode::Difference => "difference",
        BlendMode::Darken => "darken",
        BlendMode::Lighten => "lighten",
        BlendMode::Dodge => "dodge",
        BlendMode::Burn => "burn",
        BlendMode::Divide => "divide",
        BlendMode::LinearLight => "linear light",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "colour",
        BlendMode::Value => "value",
    }
}
