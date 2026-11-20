use rikka_model_core::NodeId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::asset::PieceKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VertexKey {
    pub node: NodeId,
    pub vertex: u32,
}

impl VertexKey {
    #[must_use]
    pub fn new(node: NodeId, vertex: u32) -> VertexKey {
        VertexKey { node, vertex }
    }
}

impl From<VertexKey> for PieceKey {
    fn from(key: VertexKey) -> PieceKey {
        PieceKey {
            node: key.node,
            vertex: key.vertex,
        }
    }
}

impl From<PieceKey> for VertexKey {
    fn from(key: PieceKey) -> VertexKey {
        VertexKey {
            node: key.node,
            vertex: key.vertex,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct KeyRepr {
    node: u32,
    vertex: u32,
}

impl Serialize for VertexKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        KeyRepr {
            node: self.node.0,
            vertex: self.vertex,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VertexKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<VertexKey, D::Error> {
        let KeyRepr { node, vertex } = KeyRepr::deserialize(deserializer)?;
        Ok(VertexKey::new(NodeId(node), vertex))
    }
}

impl Serialize for PieceKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        VertexKey::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PieceKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<PieceKey, D::Error> {
        VertexKey::deserialize(deserializer).map(PieceKey::from)
    }
}
