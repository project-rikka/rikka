use rikka_renderer_3d::{Hint as SchemaHint, Literal, ParameterKind, TextureKind};
use rikka_shader_graph::{
    Constant, Graph, Hint, Input, Library, NodeId, NodeKind, Port, PortDefault, PortRef, PortType,
    Scope, SocketValue, ValueType,
};

use crate::canvas_data::ports_of;
use crate::traits::AssetFilter;

#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Number {
        range: Option<(f32, f32)>,
        hard: bool,
        integer: bool,
        angle: bool,
    },
    Vector(usize),
    Color {
        alpha: bool,
    },
    Toggle,
    Options(Vec<String>),
    Asset(AssetFilter),
    Text,
    Link,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinkSource {
    pub node: NodeId,
    pub title: String,
    pub port: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fallback {
    Value(Constant),
    Context(Input),
    Required,
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SocketState {
    Linked(Vec<LinkSource>),
    Set(SocketValue),
    Default(Fallback),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SocketRow {
    pub port: String,
    pub ty: Option<ValueType>,
    pub many: bool,
    pub field: Field,
    pub state: SocketState,
}

impl SocketRow {
    pub fn fallback_value(&self) -> Option<SocketValue> {
        match &self.state {
            SocketState::Default(Fallback::Value(constant)) => Some(SocketValue::Number(*constant)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterRow {
    pub name: String,
    pub label: String,
    pub kind: ParameterKind,
    pub field: Field,
    pub default: Option<SocketValue>,
}

fn field(port: &Port, ty: Option<ValueType>, kind: &NodeKind) -> Field {
    let width = ty.and_then(ValueType::width).unwrap_or(1);
    let number = |range: Option<(f32, f32)>, hard: bool| Field::Number {
        range,
        hard,
        integer: false,
        angle: false,
    };
    match (port.hint, ty) {
        (_, Some(ValueType::Shader | ValueType::Image | ValueType::Record(_))) => Field::Link,
        (Hint::Text, _) => Field::Text,
        (Hint::Texture(kind), _) => Field::Asset(AssetFilter::Texture(kind)),
        (Hint::File(extensions), _) => Field::Asset(AssetFilter::Files(extensions)),
        (_, Some(ValueType::Texture)) => Field::Asset(AssetFilter::Texture(match kind {
            NodeKind::Texture(kind) => *kind,
            _ => TextureKind::Colour,
        })),
        (Hint::Color, _) if width >= 3 => Field::Color { alpha: width == 4 },
        (_, _) if width > 1 => Field::Vector(width),
        (Hint::Toggle, _) => Field::Toggle,
        (Hint::Options(options), _) => {
            Field::Options(options.iter().map(|option| (*option).to_owned()).collect())
        }
        (Hint::Range(low, high), _) => number(Some((low, high)), true),
        (Hint::Soft(low, high), _) => number(Some((low, high)), false),
        (Hint::Integer, _) => Field::Number {
            range: None,
            hard: false,
            integer: true,
            angle: false,
        },
        (Hint::Angle, _) => Field::Number {
            range: None,
            hard: false,
            integer: false,
            angle: true,
        },
        (Hint::None | Hint::Color, _) => number(None, false),
    }
}

pub fn socket_rows(
    graph: &Graph,
    library: &Library,
    scope: &Scope,
    node: NodeId,
) -> Vec<SocketRow> {
    let Some(body) = graph.scope(scope) else {
        return Vec::new();
    };
    let Some(held) = body.node(node) else {
        return Vec::new();
    };
    let report = graph.validate(library);
    let ports = ports_of(graph, library, &report, scope, node);
    ports
        .inputs
        .iter()
        .map(|port| {
            let ty = report
                .input_type(scope, node, &port.name)
                .or(match port.ty {
                    PortType::Value(ty) => Some(ty),
                    PortType::Dynamic => None,
                });
            let to = PortRef::new(node, port.name.clone());
            let sources: Vec<LinkSource> = body
                .sources(&to)
                .filter_map(|from| {
                    Some(LinkSource {
                        node: from.node,
                        title: body.node(from.node)?.kind.title(library),
                        port: from.port.clone(),
                    })
                })
                .collect();
            let state = match (sources.is_empty(), held.inputs.get(&port.name)) {
                (false, _) => SocketState::Linked(sources),
                (true, Some(value)) => SocketState::Set(value.clone()),
                (true, None) => SocketState::Default(match port.default {
                    PortDefault::Value(constant) => Fallback::Value(constant),
                    PortDefault::Input(input) => Fallback::Context(input),
                    PortDefault::Required => Fallback::Required,
                    PortDefault::Empty => Fallback::Empty,
                }),
            };
            SocketRow {
                port: port.name.clone(),
                ty,
                many: port.many,
                field: field(port, ty, &held.kind),
                state,
            }
        })
        .collect()
}

fn literal_value(literal: Literal) -> Option<SocketValue> {
    let constant = match literal {
        Literal::Float(value) => Constant::scalar(value),
        Literal::Vec2([x, y]) => Constant::vec2(x, y),
        Literal::Vec3([x, y, z]) => Constant::vec3(x, y, z),
        Literal::Vec4([x, y, z, w]) => Constant::vec4(x, y, z, w),
        Literal::Int(value) => Constant::scalar(value as f32),
        Literal::UInt(value) | Literal::Enum(value) => Constant::scalar(value as f32),
        Literal::Bool(on) => Constant::scalar(if on { 1.0 } else { 0.0 }),
        Literal::Texture => return None,
    };
    Some(SocketValue::Number(constant))
}

pub fn parameter_rows(graph: &Graph) -> Vec<ParameterRow> {
    graph
        .parameters()
        .iter()
        .filter(|decl| !decl.hints.contains(&SchemaHint::Hidden))
        .map(|decl| {
            let range = decl
                .range
                .map(|bounds| ((bounds.low, bounds.high), true))
                .or(decl
                    .soft_range
                    .map(|bounds| ((bounds.low, bounds.high), false)));
            let number = |integer: bool| Field::Number {
                range: range.map(|(range, _)| range),
                hard: range.is_some_and(|(_, hard)| hard),
                integer,
                angle: decl.hints.contains(&SchemaHint::Angle),
            };
            let field = match &decl.kind {
                ParameterKind::Float => number(false),
                ParameterKind::Int | ParameterKind::UInt => number(true),
                ParameterKind::Vec2 => Field::Vector(2),
                ParameterKind::Vec3 => Field::Vector(3),
                ParameterKind::Vec4 => Field::Vector(4),
                ParameterKind::Color { alpha, .. } => Field::Color { alpha: *alpha },
                ParameterKind::Bool => Field::Toggle,
                ParameterKind::Enum { options } => Field::Options(options.clone()),
                ParameterKind::Texture { kind, .. } => Field::Asset(AssetFilter::Texture(*kind)),
            };
            ParameterRow {
                name: decl.name.clone(),
                label: decl.label.clone().unwrap_or_else(|| decl.name.clone()),
                kind: decl.kind.clone(),
                field,
                default: decl.default.and_then(literal_value),
            }
        })
        .collect()
}
