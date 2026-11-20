use std::fmt;

use crate::graph::{CommentId, GraphKind, GroupId, NodeId, Scope};
use crate::types::ValueType;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct At {
    pub scope: Scope,
    pub node: Option<NodeId>,
    pub port: Option<String>,
}

impl At {
    pub(crate) fn graph() -> Self {
        Self {
            scope: Scope::Main,
            node: None,
            port: None,
        }
    }

    pub(crate) fn scope(scope: &Scope) -> Self {
        Self {
            scope: scope.clone(),
            node: None,
            port: None,
        }
    }

    pub(crate) fn node(scope: &Scope, node: NodeId) -> Self {
        Self {
            scope: scope.clone(),
            node: Some(node),
            port: None,
        }
    }

    pub(crate) fn port(scope: &Scope, node: NodeId, port: &str) -> Self {
        Self {
            scope: scope.clone(),
            node: Some(node),
            port: Some(port.to_owned()),
        }
    }
}

impl fmt::Display for At {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        match &self.scope {
            Scope::Main => {}
            Scope::Function(name) => parts.push(format!("function {name}")),
        }
        if let Some(node) = self.node {
            parts.push(format!("node {node}"));
        }
        if let Some(port) = &self.port {
            parts.push(format!("port {port}"));
        }
        if parts.is_empty() {
            formatter.write_str("graph")
        } else {
            formatter.write_str(&parts.join(", "))
        }
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum Problem {
    #[error("there is no node {0}")]
    UnknownNode(NodeId),
    #[error("there is no group {0}")]
    UnknownGroup(GroupId),
    #[error("there is no comment {0}")]
    UnknownComment(CommentId),
    #[error("there is no node kind {0}")]
    UnknownKind(String),
    #[error("{0} is already taken")]
    Taken(String),
    #[error("positions must be finite and sizes finite and positive")]
    InvalidLayout,
    #[error("there is no input {0}")]
    UnknownInput(String),
    #[error("there is no output {0}")]
    UnknownOutput(String),
    #[error("the input is connected twice")]
    AlreadyConnected,
    #[error("cannot widen {from} to {to}")]
    Widen { from: ValueType, to: ValueType },
    #[error("cannot pass {from} where {to} is expected")]
    Mismatch { from: ValueType, to: ValueType },
    #[error("the input holds {found} where {to} is expected")]
    Held { found: &'static str, to: ValueType },
    #[error("the node feeds back into itself")]
    Cycle,
    #[error("constants must be finite")]
    NotFinite,
    #[error("there is no parameter {0}")]
    UnknownParameter(String),
    #[error("there is no function {0}")]
    UnknownFunction(String),
    #[error("function {0} calls itself")]
    Recursion(String),
    #[error("function inputs and outputs belong inside a function")]
    OutsideFunction,
    #[error("the function has no port {0}")]
    UnknownFunctionPort(String),
    #[error("function output {0} is set twice")]
    DuplicateFunctionOutput(String),
    #[error("{} graphs have no such node", .0.name())]
    WrongGraph(GraphKind),
    #[error("the output belongs to the main graph, not a function")]
    OutputInFunction,
    #[error("the graph has two outputs")]
    DuplicateOutput,
    #[error("{0} is not a swizzle of one to four of xyzw or rgba")]
    InvalidSwizzle(String),
    #[error("the swizzle reads component {component} of a {ty}")]
    SwizzleRange { component: char, ty: ValueType },
    #[error("ramps and curves need at least one finite point")]
    EmptyRamp,
    #[error("{what} is not available for {stage}")]
    Unavailable { what: String, stage: &'static str },
    #[error("the input defaults to {what}, which {stage} lacks; connect it")]
    DefaultUnavailable { what: String, stage: &'static str },
    #[error("a value that differs per light only feeds the light input of a lighting node")]
    PerLight,
    #[error("a value read from the surroundings only feeds the ambient input of a lighting node")]
    PerAmbient,
    #[error("values per light and values from the surroundings cannot be combined")]
    MixedLighting,
    #[error("the value changes per pixel, but this input takes one value for the whole frame")]
    PerPixel,
    #[error("{0} is not a valid name")]
    InvalidName(String),
    #[error("{0} is declared twice")]
    Duplicate(String),
    #[error("parameter {0}: the default does not fit its kind")]
    DefaultMismatch(String),
    #[error("parameter {0}: the role does not fit its kind")]
    RoleMismatch(String),
    #[error("parameter {0}: the name is reserved")]
    Reserved(String),
    #[error("only material graphs declare parameters")]
    Parameters,
    #[error("parameter {0}: texts may not hold quotes, backslashes, bars or line breaks")]
    UnsafeText(String),
    #[error("the input needs a connection")]
    Required,
    #[error("the graph needs an output")]
    MissingOutput,
    #[error("function output {0} is never set")]
    MissingFunctionOutput(String),
    #[error(
        "the displacement moves vertices without a float parameter of the displacement role \
         bounding how far"
    )]
    Unbounded,
}

impl Problem {
    pub fn is_incomplete(&self) -> bool {
        matches!(
            self,
            Self::Required
                | Self::DefaultUnavailable { .. }
                | Self::MissingOutput
                | Self::MissingFunctionOutput(_)
                | Self::Unbounded
        )
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("{at}: {problem}")]
pub struct GraphError {
    pub at: At,
    pub problem: Problem,
}

impl GraphError {
    pub(crate) fn new(at: At, problem: Problem) -> Self {
        Self { at, problem }
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("invalid graph: {}", join(.0))]
pub struct Error(pub Vec<GraphError>);

impl Error {
    pub fn errors(&self) -> &[GraphError] {
        &self.0
    }
}

fn join(errors: &[GraphError]) -> String {
    errors
        .iter()
        .map(GraphError::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}
