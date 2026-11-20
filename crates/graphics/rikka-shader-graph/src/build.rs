use std::collections::{BTreeMap, BTreeSet};

use rikka_slang::TextureKind;

use crate::error::{Error, GraphError};
use crate::graph::{Body, Function, Graph, GraphKind, Group, NodeId, PortRef, Scope};
use crate::library::Library;
use crate::node::{BlendMode, Curve, Input, Layer, NodeKind, Ramp};
use crate::ops::Op;
use crate::parameter::ParameterDecl;
use crate::types::{Constant, SocketValue};

const COLUMN: f32 = 280.0;
const HEADER: f32 = 56.0;
const ROW: f32 = 24.0;
const GAP: f32 = 28.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Wire {
    node: NodeId,
    port: String,
}

impl Wire {
    pub fn node(&self) -> NodeId {
        self.node
    }

    pub fn port(&self) -> &str {
        &self.port
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Wire(Wire),
    Value(SocketValue),
}

impl From<Wire> for Arg {
    fn from(wire: Wire) -> Self {
        Self::Wire(wire)
    }
}

impl From<&Wire> for Arg {
    fn from(wire: &Wire) -> Self {
        Self::Wire(wire.clone())
    }
}

impl From<f32> for Arg {
    fn from(value: f32) -> Self {
        Self::Value(value.into())
    }
}

impl From<[f32; 2]> for Arg {
    fn from(value: [f32; 2]) -> Self {
        Self::Value(value.into())
    }
}

impl From<[f32; 3]> for Arg {
    fn from(value: [f32; 3]) -> Self {
        Self::Value(value.into())
    }
}

impl From<[f32; 4]> for Arg {
    fn from(value: [f32; 4]) -> Self {
        Self::Value(value.into())
    }
}

impl From<&str> for Arg {
    fn from(path: &str) -> Self {
        Self::Value(path.into())
    }
}

#[derive(Debug)]
pub struct Builder<'l> {
    graph: Graph,
    library: &'l Library,
    scope: Scope,
    errors: Vec<GraphError>,
    frames: Vec<(Scope, Group)>,
    open: Vec<usize>,
}

impl<'l> Builder<'l> {
    pub fn new(kind: GraphKind, library: &'l Library) -> Self {
        Self {
            graph: Graph::new(kind),
            library,
            scope: Scope::Main,
            errors: Vec::new(),
            frames: Vec::new(),
            open: Vec::new(),
        }
    }

    fn keep<T>(&mut self, result: Result<T, GraphError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    pub fn node(&mut self, kind: NodeKind) -> NodeId {
        let added = self
            .graph
            .edit(self.library, self.scope.clone())
            .add_node(kind, [0.0, 0.0])
            .map(|change| change.value);
        let id = self.keep(added).unwrap_or(NodeId(u32::MAX));
        if let Some(open) = self.open.last()
            && self.frames[*open].0 == self.scope
        {
            self.frames[*open].1.nodes.insert(id);
        }
        id
    }

    pub fn set(&mut self, node: NodeId, port: &str, arg: impl Into<Arg>) -> &mut Self {
        let mut editor = self.graph.edit(self.library, self.scope.clone());
        let done = match arg.into() {
            Arg::Wire(wire) => {
                editor.connect(PortRef::new(wire.node, wire.port), PortRef::new(node, port))
            }
            Arg::Value(value) => editor.set_input(node, port, Some(value)),
        };
        self.keep(done);
        self
    }

    pub fn with<const N: usize>(&mut self, kind: NodeKind, inputs: [(&str, Arg); N]) -> NodeId {
        let node = self.node(kind);
        for (port, arg) in inputs {
            self.set(node, port, arg);
        }
        node
    }

    pub fn out(&self, node: NodeId, port: &str) -> Wire {
        Wire {
            node,
            port: port.to_owned(),
        }
    }

    pub fn op<const N: usize>(&mut self, op: Op, args: [Arg; N]) -> Wire {
        let info = op.info();
        let node = self.node(NodeKind::Op(op));
        for (port, arg) in info.inputs.iter().zip(args) {
            self.set(node, port.name, arg);
        }
        self.out(node, info.outputs[0].name)
    }

    pub fn add(&mut self, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
        self.op(Op::Add, [a.into(), b.into()])
    }

    pub fn subtract(&mut self, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
        self.op(Op::Subtract, [a.into(), b.into()])
    }

    pub fn multiply(&mut self, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
        self.op(Op::Multiply, [a.into(), b.into()])
    }

    pub fn lerp(&mut self, a: impl Into<Arg>, b: impl Into<Arg>, t: impl Into<Arg>) -> Wire {
        self.op(Op::Lerp, [a.into(), b.into(), t.into()])
    }

    pub fn smoothstep(
        &mut self,
        low: impl Into<Arg>,
        high: impl Into<Arg>,
        value: impl Into<Arg>,
    ) -> Wire {
        self.op(Op::Smoothstep, [low.into(), high.into(), value.into()])
    }

    pub fn dot(&mut self, a: impl Into<Arg>, b: impl Into<Arg>) -> Wire {
        self.op(Op::Dot, [a.into(), b.into()])
    }

    pub fn saturate(&mut self, value: impl Into<Arg>) -> Wire {
        self.op(Op::Saturate, [value.into()])
    }

    pub fn constant(&mut self, value: Constant) -> Wire {
        let node = self.node(NodeKind::Constant(value));
        self.out(node, "value")
    }

    pub fn input(&mut self, input: Input) -> Wire {
        let node = self.node(NodeKind::Input(input));
        self.out(node, "value")
    }

    pub fn swizzle(&mut self, value: impl Into<Arg>, mask: &str) -> Wire {
        let node = self.with(
            NodeKind::Swizzle(mask.to_owned()),
            [("value", value.into())],
        );
        self.out(node, "value")
    }

    pub fn parameter(&mut self, parameter: ParameterDecl) -> Wire {
        let name = parameter.name.clone();
        if self.graph.parameter(&name).is_none() {
            let added = self.graph.add_parameter(self.library, parameter);
            self.keep(added);
        }
        let node = self.node(NodeKind::Parameter(name));
        self.out(node, "value")
    }

    pub fn texture<const N: usize>(
        &mut self,
        kind: TextureKind,
        inputs: [(&str, Arg); N],
    ) -> NodeId {
        self.with(NodeKind::Texture(kind), inputs)
    }

    pub fn ramp(&mut self, ramp: Ramp, position: impl Into<Arg>) -> Wire {
        let node = self.with(NodeKind::ColorRamp(ramp), [("position", position.into())]);
        self.out(node, "value")
    }

    pub fn curve(&mut self, curve: Curve, value: impl Into<Arg>) -> Wire {
        let node = self.with(NodeKind::Curve(curve), [("value", value.into())]);
        self.out(node, "value")
    }

    pub fn blend(
        &mut self,
        mode: BlendMode,
        base: impl Into<Arg>,
        blend: impl Into<Arg>,
        opacity: impl Into<Arg>,
    ) -> Wire {
        let node = self.with(
            NodeKind::Blend(mode),
            [
                ("base", base.into()),
                ("blend", blend.into()),
                ("opacity", opacity.into()),
            ],
        );
        self.out(node, "value")
    }

    pub fn layer(&mut self, layer: Layer) -> Wire {
        let node = self.node(NodeKind::Layer(layer));
        self.out(node, "image")
    }

    pub fn external<const N: usize>(&mut self, id: &str, inputs: [(&str, Arg); N]) -> NodeId {
        self.with(NodeKind::Extern(id.to_owned()), inputs)
    }

    pub fn shader<const N: usize>(&mut self, kind: NodeKind, inputs: [(&str, Arg); N]) -> Wire {
        let node = self.with(kind, inputs);
        self.out(node, "shader")
    }

    pub fn call<const N: usize>(&mut self, function: &str, inputs: [(&str, Arg); N]) -> NodeId {
        self.with(NodeKind::Call(function.to_owned()), inputs)
    }

    pub fn function(
        &mut self,
        name: &str,
        function: Function,
        body: impl FnOnce(&mut Self),
    ) -> &mut Self {
        let added = self.graph.add_function(self.library, name, function);
        self.keep(added);
        let outer = std::mem::replace(&mut self.scope, Scope::Function(name.to_owned()));
        body(self);
        self.scope = outer;
        self
    }

    pub fn function_input(&mut self, name: &str) -> Wire {
        let node = self.node(NodeKind::FunctionInput(name.to_owned()));
        self.out(node, "value")
    }

    pub fn function_output(&mut self, name: &str, value: impl Into<Arg>) -> &mut Self {
        self.with(
            NodeKind::FunctionOutput(name.to_owned()),
            [("value", value.into())],
        );
        self
    }

    pub fn output<const N: usize>(&mut self, inputs: [(&str, Arg); N]) -> NodeId {
        self.with(NodeKind::Output, inputs)
    }

    pub fn frame<T>(&mut self, title: &str, body: impl FnOnce(&mut Self) -> T) -> T {
        self.frames.push((
            self.scope.clone(),
            Group {
                title: title.to_owned(),
                nodes: BTreeSet::new(),
                color: None,
            },
        ));
        self.open.push(self.frames.len() - 1);
        let made = body(self);
        self.open.pop();
        made
    }

    pub fn finish(mut self) -> Result<Graph, Error> {
        for (scope, group) in std::mem::take(&mut self.frames) {
            if group.nodes.is_empty() {
                continue;
            }
            let added = self
                .graph
                .edit(self.library, scope)
                .add_group(group)
                .map(|_| ());
            self.keep(added);
        }
        if !self.errors.is_empty() {
            return Err(Error(self.errors));
        }
        let report = self.graph.validate(self.library);
        if !report.is_valid() {
            return Err(Error(report.errors));
        }
        self.graph.arrange(self.library);
        Ok(self.graph)
    }
}

impl Graph {
    pub fn arrange(&mut self, library: &Library) {
        let scopes: Vec<Scope> = self.scopes().map(|(scope, _)| scope).collect();
        for scope in scopes {
            let rows: BTreeMap<NodeId, usize> = self
                .scope(&scope)
                .map(|body| {
                    body.nodes()
                        .iter()
                        .map(|(id, node)| {
                            let rows = self
                                .ports(library, &scope, &node.kind)
                                .map_or(1, |ports| ports.inputs.len().max(ports.outputs.len()));
                            (*id, rows)
                        })
                        .collect()
                })
                .unwrap_or_default();
            if let Some(body) = self.scope_mut(&scope) {
                place(body, &rows);
            }
        }
    }
}

fn place(body: &mut Body, rows: &BTreeMap<NodeId, usize>) {
    let mut users: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for connection in &body.connections {
        users
            .entry(connection.from.node)
            .or_default()
            .push(connection.to.node);
    }
    let mut depth: BTreeMap<NodeId, usize> = BTreeMap::new();
    fn sink_distance(
        id: NodeId,
        users: &BTreeMap<NodeId, Vec<NodeId>>,
        depth: &mut BTreeMap<NodeId, usize>,
        guard: usize,
    ) -> usize {
        if let Some(known) = depth.get(&id) {
            return *known;
        }
        let distance = match guard {
            0 => 0,
            _ => users
                .get(&id)
                .into_iter()
                .flatten()
                .map(|user| sink_distance(*user, users, depth, guard - 1) + 1)
                .max()
                .unwrap_or(0),
        };
        depth.insert(id, distance);
        distance
    }
    let guard = body.nodes.len();
    let ids: Vec<NodeId> = body.nodes.keys().copied().collect();
    for id in &ids {
        sink_distance(*id, &users, &mut depth, guard);
    }
    let deepest = depth.values().copied().max().unwrap_or(0);
    let mut columns: BTreeMap<usize, Vec<NodeId>> = BTreeMap::new();
    for id in &ids {
        columns.entry(depth[id]).or_default().push(*id);
    }
    let mut middle: BTreeMap<NodeId, f32> = BTreeMap::new();
    for (column, members) in &columns {
        let mut ordered: Vec<(f32, NodeId)> = members
            .iter()
            .map(|id| {
                let pulls: Vec<f32> = users
                    .get(id)
                    .into_iter()
                    .flatten()
                    .filter_map(|user| middle.get(user).copied())
                    .collect();
                let pull = match pulls.is_empty() {
                    true => f32::MAX,
                    false => pulls.iter().sum::<f32>() / pulls.len() as f32,
                };
                (pull, *id)
            })
            .collect();
        ordered.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let heights: Vec<f32> = ordered
            .iter()
            .map(|(_, id)| HEADER + ROW * rows.get(id).copied().unwrap_or(1) as f32)
            .collect();
        let total: f32 = heights.iter().sum::<f32>() + GAP * heights.len().saturating_sub(1) as f32;
        let mut y = -total / 2.0;
        for ((_, id), height) in ordered.iter().zip(&heights) {
            let x = (deepest - column) as f32 * COLUMN;
            if let Some(node) = body.nodes.get_mut(id) {
                node.position = [x, y];
            }
            middle.insert(*id, y + height / 2.0);
            y += height + GAP;
        }
    }
}
