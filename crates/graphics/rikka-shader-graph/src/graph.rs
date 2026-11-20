use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::node::NodeKind;
use crate::parameter::ParameterDecl;
use crate::types::{Constant, SocketValue, ValueType};

struct ItemIdVisitor;

impl Visitor<'_> for ItemIdVisitor {
    type Value = u32;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an item id")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<u32, E> {
        u32::try_from(value).map_err(|_| E::invalid_value(Unexpected::Unsigned(value), &self))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<u32, E> {
        u32::try_from(value).map_err(|_| E::invalid_value(Unexpected::Signed(value), &self))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<u32, E> {
        text.parse()
            .map_err(|_| E::invalid_value(Unexpected::Str(text), &self))
    }
}

macro_rules! item_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub(crate) u32);

        impl $name {
            pub fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserializer.deserialize_any(ItemIdVisitor).map(Self)
            }
        }
    };
}

item_id!(NodeId);
item_id!(GroupId);
item_id!(CommentId);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    #[serde(default)]
    pub position: [f32; 2],
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, SocketValue>,
}

impl Node {
    pub fn new(kind: NodeKind, position: [f32; 2]) -> Self {
        Self {
            kind,
            position,
            inputs: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PortRef {
    pub node: NodeId,
    pub port: String,
}

impl PortRef {
    pub fn new(node: NodeId, port: impl Into<String>) -> Self {
        Self {
            node,
            port: port.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Connection {
    pub from: PortRef,
    pub to: PortRef,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub title: String,
    pub nodes: BTreeSet<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f32; 4]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    pub text: String,
    pub position: [f32; 2],
    pub size: [f32; 2],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Body {
    #[serde(default)]
    pub(crate) nodes: BTreeMap<NodeId, Node>,
    #[serde(default)]
    pub(crate) connections: BTreeSet<Connection>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) groups: BTreeMap<GroupId, Group>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) comments: BTreeMap<CommentId, Comment>,
}

impl Body {
    pub fn nodes(&self) -> &BTreeMap<NodeId, Node> {
        &self.nodes
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn connections(&self) -> &BTreeSet<Connection> {
        &self.connections
    }

    pub fn groups(&self) -> &BTreeMap<GroupId, Group> {
        &self.groups
    }

    pub fn comments(&self) -> &BTreeMap<CommentId, Comment> {
        &self.comments
    }

    pub fn source<'a>(&'a self, to: &'a PortRef) -> Option<&'a PortRef> {
        self.sources(to).next()
    }

    pub fn sources<'a>(&'a self, to: &'a PortRef) -> impl Iterator<Item = &'a PortRef> {
        self.connections
            .iter()
            .filter(move |connection| &connection.to == to)
            .map(|connection| &connection.from)
    }

    pub fn output(&self) -> Option<NodeId> {
        self.nodes
            .iter()
            .find(|(_, node)| node.kind == NodeKind::Output)
            .map(|(id, _)| *id)
    }

    pub(crate) fn fresh(&self) -> NodeId {
        NodeId(self.nodes.keys().next_back().map_or(0, |id| id.0 + 1))
    }

    pub(crate) fn fresh_group(&self) -> GroupId {
        GroupId(self.groups.keys().next_back().map_or(0, |id| id.0 + 1))
    }

    pub(crate) fn fresh_comment(&self) -> CommentId {
        CommentId(self.comments.keys().next_back().map_or(0, |id| id.0 + 1))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionPort {
    pub name: String,
    pub ty: ValueType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Constant>,
}

impl FunctionPort {
    pub fn new(name: impl Into<String>, ty: ValueType) -> Self {
        Self {
            name: name.into(),
            ty,
            default: None,
        }
    }

    pub fn with_default(mut self, default: Constant) -> Self {
        self.default = Some(default);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Function {
    #[serde(default)]
    pub inputs: Vec<FunctionPort>,
    #[serde(default)]
    pub outputs: Vec<FunctionPort>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
    #[serde(default)]
    pub body: Body,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphKind {
    Material,
    World,
    Composite,
}

impl GraphKind {
    pub const ALL: [Self; 3] = [Self::Material, Self::World, Self::Composite];

    pub fn name(self) -> &'static str {
        match self {
            Self::Material => "material",
            Self::World => "world",
            Self::Composite => "composite",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Main,
    Function(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SocketRef {
    pub scope: Scope,
    pub node: NodeId,
    pub port: String,
}

impl SocketRef {
    pub fn new(scope: Scope, node: NodeId, port: impl Into<String>) -> Self {
        Self {
            scope,
            node,
            port: port.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub(crate) kind: GraphKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) parameters: Vec<ParameterDecl>,
    #[serde(default)]
    pub(crate) body: Body,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) functions: BTreeMap<String, Function>,
}

impl Graph {
    pub fn new(kind: GraphKind) -> Self {
        Self {
            kind,
            parameters: Vec::new(),
            body: Body::default(),
            functions: BTreeMap::new(),
        }
    }

    pub fn kind(&self) -> GraphKind {
        self.kind
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn parameters(&self) -> &[ParameterDecl] {
        &self.parameters
    }

    pub fn parameter(&self, name: &str) -> Option<&ParameterDecl> {
        self.parameters
            .iter()
            .find(|parameter| parameter.name == name)
    }

    pub fn functions(&self) -> &BTreeMap<String, Function> {
        &self.functions
    }

    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.get(name)
    }

    pub fn scope(&self, scope: &Scope) -> Option<&Body> {
        match scope {
            Scope::Main => Some(&self.body),
            Scope::Function(name) => self.functions.get(name).map(|function| &function.body),
        }
    }

    pub fn socket(&self, at: &SocketRef) -> Option<&SocketValue> {
        self.scope(&at.scope)?.node(at.node)?.inputs.get(&at.port)
    }

    pub(crate) fn scope_mut(&mut self, scope: &Scope) -> Option<&mut Body> {
        match scope {
            Scope::Main => Some(&mut self.body),
            Scope::Function(name) => self
                .functions
                .get_mut(name)
                .map(|function| &mut function.body),
        }
    }

    pub fn scopes(&self) -> impl Iterator<Item = (Scope, &Body)> {
        std::iter::once((Scope::Main, &self.body)).chain(
            self.functions
                .iter()
                .map(|(name, function)| (Scope::Function(name.clone()), &function.body)),
        )
    }

    pub(crate) fn function_index(&self, name: &str) -> Option<usize> {
        self.functions.keys().position(|known| known == name)
    }
}
