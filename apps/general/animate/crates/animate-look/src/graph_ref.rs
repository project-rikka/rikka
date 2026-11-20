use std::fmt;

use rikka_shader_graph::{Edit, GraphKind, SocketRef, SocketValue};
use serde::{Deserialize, Serialize};

use crate::error::Error;

pub const MAX_MATERIAL_KEY: usize = 48;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphRef {
    World,
    Composite,
    Material(String),
}

impl GraphRef {
    #[must_use]
    pub fn kind(&self) -> GraphKind {
        match self {
            Self::World => GraphKind::World,
            Self::Composite => GraphKind::Composite,
            Self::Material(_) => GraphKind::Material,
        }
    }

    #[must_use]
    pub fn material(&self) -> Option<&str> {
        match self {
            Self::Material(key) => Some(key),
            Self::World | Self::Composite => None,
        }
    }
}

impl fmt::Display for GraphRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::World => f.write_str("world"),
            Self::Composite => f.write_str("composite"),
            Self::Material(key) => write!(f, "material {key}"),
        }
    }
}

pub fn check_material_key(key: &str) -> Result<(), Error> {
    let valid = !key.is_empty()
        && key.chars().count() <= MAX_MATERIAL_KEY
        && key.trim() == key
        && key
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '_' | '-' | '.'));
    match valid {
        true => Ok(()),
        false => Err(Error::InvalidKey(key.to_owned())),
    }
}

#[must_use]
pub fn fresh_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    let base: String = base.chars().take(MAX_MATERIAL_KEY - 4).collect();
    let base = base.trim();
    (1..)
        .map(|number| match number {
            1 => base.to_owned(),
            _ => format!("{base} {number}"),
        })
        .find(|key| !taken(key))
        .expect("some numbered name is free")
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SocketKey {
    pub graph: GraphRef,
    pub socket: SocketRef,
}

impl SocketKey {
    #[must_use]
    pub fn new(graph: GraphRef, socket: SocketRef) -> Self {
        Self { graph, socket }
    }

    #[must_use]
    pub fn edit(&self, value: Option<SocketValue>) -> Edit {
        Edit::SetInput {
            scope: self.socket.scope.clone(),
            id: self.socket.node,
            port: self.socket.port.clone(),
            value,
        }
    }
}

impl fmt::Display for SocketKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} node {} {}",
            self.graph, self.socket.node, self.socket.port
        )
    }
}
