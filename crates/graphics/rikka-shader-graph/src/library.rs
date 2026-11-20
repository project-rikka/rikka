use std::collections::BTreeMap;

use crate::graph::GraphKind;
use crate::node::PortSpec;
use crate::ops::Category;
use crate::types::Constant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hosts {
    pub material: bool,
    pub world: bool,
    pub composite: bool,
}

impl Hosts {
    pub const MATERIAL: Self = Self {
        material: true,
        world: false,
        composite: false,
    };
    pub const WORLD: Self = Self {
        material: false,
        world: true,
        composite: false,
    };
    pub const COMPOSITE: Self = Self {
        material: false,
        world: false,
        composite: true,
    };
    pub const ANY: Self = Self {
        material: true,
        world: true,
        composite: true,
    };

    pub fn holds(self, kind: GraphKind) -> bool {
        match kind {
            GraphKind::Material => self.material,
            GraphKind::World => self.world,
            GraphKind::Composite => self.composite,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Albedo,
    Alpha,
    Normal,
    Occlusion,
    Emission,
    Data(&'static str),
    Flag(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bypass {
    Input(&'static str),
    Constant(Constant),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub input: &'static str,
    pub bypass: Bypass,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCode {
    pub code: &'static str,
    pub ty: &'static str,
    pub outputs: Vec<(&'static str, &'static str)>,
    pub gate: Option<Gate>,
}

impl FunctionCode {
    pub fn value(code: &'static str) -> Self {
        Self {
            code,
            ty: "",
            outputs: Vec::new(),
            gate: None,
        }
    }

    pub fn packed(
        code: &'static str,
        ty: &'static str,
        outputs: &[(&'static str, &'static str)],
    ) -> Self {
        Self {
            code,
            ty,
            outputs: outputs.to_vec(),
            gate: None,
        }
    }

    pub fn gated(mut self, input: &'static str, bypass: Bypass) -> Self {
        self.gate = Some(Gate { input, bypass });
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClosureCode {
    pub model: &'static str,
    pub data: &'static str,
    pub fields: Vec<(&'static str, Field)>,
    pub features: Vec<(&'static str, u32)>,
    pub subsurface: Option<&'static str>,
    pub transmission: Option<&'static str>,
    pub unscattered: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Behavior {
    Function(FunctionCode),
    Closure(ClosureCode),
    Filter,
    Record,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub category: Category,
    pub doc: &'static str,
    pub hosts: Hosts,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    pub behavior: Behavior,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    specs: BTreeMap<&'static str, NodeSpec>,
    world: Vec<PortSpec>,
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, spec: NodeSpec) -> Self {
        self.insert(spec);
        self
    }

    pub fn insert(&mut self, spec: NodeSpec) {
        self.specs.insert(spec.id, spec);
    }

    pub fn extend(&mut self, specs: impl IntoIterator<Item = NodeSpec>) {
        for spec in specs {
            self.insert(spec);
        }
    }

    pub fn with_world(mut self, ports: Vec<PortSpec>) -> Self {
        self.world = ports;
        self
    }

    pub fn get(&self, id: &str) -> Option<&NodeSpec> {
        self.specs.get(id)
    }

    pub fn specs(&self) -> impl Iterator<Item = &NodeSpec> {
        self.specs.values()
    }

    pub fn world(&self) -> &[PortSpec] {
        &self.world
    }
}
