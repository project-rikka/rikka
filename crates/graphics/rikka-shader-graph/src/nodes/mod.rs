mod closures;
mod color;
mod face;
mod light;
mod outline;
mod rim;
mod shade;
mod specular;
mod texture;

use rikka_slang::TextureKind;

use crate::library::{Behavior, Bypass, FunctionCode, Gate, Hosts, Library, NodeSpec};
use crate::node::{DirectionSpace, Hint, Input, PortDefault, PortSpec, PositionSpace, ViewSpace};
use crate::ops::Category;
use crate::types::{PortType, ValueType};

impl Library {
    pub fn standard() -> Self {
        let mut library = Self::new();
        library.extend(closures::specs());
        library.extend(color::specs());
        library.extend(face::specs());
        library.extend(light::specs());
        library.extend(outline::specs());
        library.extend(rim::specs());
        library.extend(shade::specs());
        library.extend(specular::specs());
        library.extend(texture::specs());
        library
    }
}

const PIXELS: Hosts = Hosts {
    material: true,
    world: false,
    composite: true,
};

const NORMAL: PortSpec = PortSpec::context(
    "normal",
    ValueType::Float3,
    Input::Normal(DirectionSpace::World),
);

const VIEW: PortSpec = PortSpec::context(
    "view",
    ValueType::Float3,
    Input::ViewDirection(ViewSpace::World),
);

const LIGHT: PortSpec = PortSpec::context("light", ValueType::Float3, Input::LightDirection);

const TANGENT: PortSpec = PortSpec::context(
    "tangent",
    ValueType::Float3,
    Input::Tangent(DirectionSpace::World),
);

const POSITION: PortSpec = PortSpec::context(
    "position",
    ValueType::Float3,
    Input::Position(PositionSpace::World),
);

const UV: PortSpec = PortSpec::context("uv", ValueType::Float2, Input::Uv);

const VERTEX_COLOR: PortSpec =
    PortSpec::context("vertex_color", ValueType::Float4, Input::VertexColor);

const CHANNELS: &[&str] = &["none", "r", "g", "b", "a"];

const DEPTHS: &[&str] = &["scene", "hair", "face"];

fn texture(name: &'static str, kind: TextureKind) -> PortSpec {
    PortSpec::empty(name, ValueType::Texture).hint(Hint::Texture(kind))
}

fn unit(name: &'static str, value: f32) -> PortSpec {
    PortSpec::number(name, value).range(0.0, 1.0)
}

fn integer(name: &'static str, value: f32) -> PortSpec {
    PortSpec::number(name, value).hint(Hint::Integer)
}

fn plane(name: &'static str, x: f32, y: f32) -> PortSpec {
    PortSpec::value(name, ValueType::Float2, crate::types::Constant::vec2(x, y))
}

fn number(name: &'static str) -> PortSpec {
    PortSpec::output(name, ValueType::Float)
}

fn spatial(name: &'static str) -> PortSpec {
    PortSpec::output(name, ValueType::Float3)
}

fn dynamic(name: &'static str) -> PortSpec {
    PortSpec::new(name, PortType::Dynamic, PortDefault::Required)
}

struct Function {
    id: &'static str,
    title: &'static str,
    category: Category,
    hosts: Hosts,
    doc: &'static str,
    gate: Option<Gate>,
}

impl Function {
    const fn new(id: &'static str, title: &'static str, doc: &'static str) -> Self {
        Self {
            id,
            title,
            category: Category::Toon,
            hosts: Hosts::MATERIAL,
            doc,
            gate: None,
        }
    }

    const fn category(mut self, category: Category) -> Self {
        self.category = category;
        self
    }

    const fn hosts(mut self, hosts: Hosts) -> Self {
        self.hosts = hosts;
        self
    }

    const fn gated(mut self, input: &'static str, bypass: Bypass) -> Self {
        self.gate = Some(Gate { input, bypass });
        self
    }

    fn value(self, inputs: &[PortSpec], output: PortSpec, code: &'static str) -> NodeSpec {
        self.spec(inputs, vec![output], FunctionCode::value(code))
    }

    fn packed(
        self,
        inputs: &[PortSpec],
        code: &'static str,
        ty: &'static str,
        outputs: &[(PortSpec, &'static str)],
    ) -> NodeSpec {
        let members: Vec<(&'static str, &'static str)> = outputs
            .iter()
            .map(|(port, member)| (port.name, *member))
            .collect();
        self.spec(
            inputs,
            outputs.iter().map(|(port, _)| *port).collect(),
            FunctionCode::packed(code, ty, &members),
        )
    }

    fn spec(self, inputs: &[PortSpec], outputs: Vec<PortSpec>, code: FunctionCode) -> NodeSpec {
        NodeSpec {
            id: self.id,
            title: self.title,
            category: self.category,
            doc: self.doc,
            hosts: self.hosts,
            inputs: inputs.to_vec(),
            outputs,
            behavior: Behavior::Function(FunctionCode {
                gate: self.gate,
                ..code
            }),
        }
    }
}
