use crate::device::Device;
use crate::graph::Graph;
use crate::node::{Node, NodeKind};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub nodes: Vec<Node>,
    pub devices: Vec<Device>,
    pub default_sink: Option<String>,
    pub default_source: Option<String>,
    pub graph: Graph,
}

impl Snapshot {
    pub fn nodes_of_kind(&self, kind: NodeKind) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |node| node.kind == kind)
    }

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.name == name)
    }

    pub fn device(&self, id: u32) -> Option<&Device> {
        self.devices.iter().find(|device| device.id == id)
    }

    pub fn default_node(&self, kind: NodeKind) -> Option<&Node> {
        let default_name = match kind {
            NodeKind::Sink => self.default_sink.as_deref(),
            NodeKind::Source => self.default_source.as_deref(),
            NodeKind::Playback | NodeKind::Recording => return None,
        };
        default_name
            .and_then(|name| self.node_by_name(name))
            .filter(|node| node.kind == kind)
            .or_else(|| self.nodes_of_kind(kind).next())
    }

    pub fn sink(&self) -> Option<&Node> {
        self.default_node(NodeKind::Sink)
    }

    pub fn source(&self) -> Option<&Node> {
        self.default_node(NodeKind::Source)
    }
}
