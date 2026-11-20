use animate_look::GraphRef;
use rikka_mmd::VmdCurve;
use rikka_shader_graph::{GraphKind, NodeId, NodeKind};

#[derive(Clone, Debug, PartialEq)]
pub enum LookQuery {
    Overview,
    Graph(GraphRef),
    Figure(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodesQuery {
    pub kind: GraphKind,
    pub search: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeRef {
    Id(NodeId),
    Named(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Given {
    Numbers(Vec<f32>),
    Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plug {
    pub node: NodeRef,
    pub port: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphOp {
    Add {
        name: Option<String>,
        kind: NodeKind,
        at: Option<[f32; 2]>,
        values: Vec<(String, Given)>,
    },
    Remove(NodeRef),
    Connect {
        from: Plug,
        to: Plug,
    },
    Disconnect {
        to: Plug,
        from: Option<Plug>,
    },
    Set {
        to: Plug,
        value: Given,
    },
    Unset(Plug),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphEdits {
    pub graph: GraphRef,
    pub ops: Vec<GraphOp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotRef {
    Index(u32),
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotPick {
    All,
    Class(String),
    Listed(Vec<SlotRef>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Assigned {
    Imported,
    Graph(String),
    Preset(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub figure: String,
    pub slots: SlotPick,
    pub material: Assigned,
    pub unique: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlotValue {
    pub figure: String,
    pub slot: SlotRef,
    pub parameter: String,
    pub value: Option<Given>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialEdits {
    pub add: Vec<(String, Option<String>)>,
    pub remove: Vec<String>,
    pub rename: Vec<(String, String)>,
    pub assign: Vec<Assignment>,
    pub values: Vec<SlotValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KeyTarget {
    Socket {
        graph: GraphRef,
        node: NodeId,
        port: String,
    },
    Slot {
        figure: String,
        slot: SlotRef,
        parameter: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeySet {
    pub target: KeyTarget,
    pub frame: u32,
    pub value: Option<Given>,
    pub curve: Option<VmdCurve>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyRemoval {
    pub target: KeyTarget,
    pub frame: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LookKeys {
    pub set: Vec<KeySet>,
    pub remove: Vec<KeyRemoval>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarterKind {
    Bare,
    Cinematic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LookReset {
    pub world: Option<StarterKind>,
    pub composite: Option<StarterKind>,
}
