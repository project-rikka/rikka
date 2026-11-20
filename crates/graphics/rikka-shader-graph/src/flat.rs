use std::collections::{BTreeMap, BTreeSet};

use crate::check::{NodeTypes, Report};
use crate::context::{Site, Taint, code_available, code_taint, input_code, input_taint};
use crate::error::{At, GraphError, Problem};
use crate::graph::{Body, Graph, NodeId, PortRef, Scope, SocketRef};
use crate::library::{Behavior, Library, NodeSpec};
use crate::node::{Input, NodeKind, PortDefault};
use crate::ops::Needs;
use crate::types::{Constant, SocketValue, ValueType};

pub(crate) type Path = Vec<NodeId>;

pub(crate) type Slot = (Path, String);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Source {
    Link(Path, String),
    Links(Vec<Slot>),
    Constant(Constant),
    Socket(SocketRef, SocketValue),
    Context(Input),
    Empty,
    Missing,
}

impl Source {
    pub(crate) fn links(&self) -> &[Slot] {
        match self {
            Self::Links(links) => links,
            _ => &[],
        }
    }
}

#[derive(Debug)]
pub(crate) struct FlatNode<'g> {
    pub kind: &'g NodeKind,
    pub scope: Scope,
    pub id: NodeId,
    pub inputs: Vec<(String, Source)>,
}

impl FlatNode<'_> {
    pub(crate) fn input(&self, port: &str) -> &Source {
        self.inputs
            .iter()
            .find(|(name, _)| name == port)
            .map_or(&Source::Missing, |(_, source)| source)
    }

    pub(crate) fn at(&self, port: Option<&str>) -> At {
        At {
            scope: self.scope.clone(),
            node: Some(self.id),
            port: port.map(str::to_owned),
        }
    }

    pub(crate) fn sources(&self) -> impl Iterator<Item = (&str, &Path, &str)> {
        self.inputs.iter().flat_map(|(port, source)| {
            let single = match source {
                Source::Link(from, output) => Some((port.as_str(), from, output.as_str())),
                _ => None,
            };
            single.into_iter().chain(
                source
                    .links()
                    .iter()
                    .map(move |(from, output)| (port.as_str(), from, output.as_str())),
            )
        })
    }
}

#[derive(Debug)]
pub(crate) struct Flat<'g> {
    pub nodes: BTreeMap<Path, FlatNode<'g>>,
    pub report: &'g Report,
    pub library: &'g Library,
    pub graph: &'g Graph,
}

impl<'g> Flat<'g> {
    pub(crate) fn new(graph: &'g Graph, library: &'g Library, report: &'g Report) -> Self {
        let mut flattener = Flattener {
            graph,
            report,
            stack: Vec::new(),
            flat: Self {
                nodes: BTreeMap::new(),
                report,
                library,
                graph,
            },
        };
        flattener.inline(&Scope::Main, graph.body(), &Vec::new(), &BTreeMap::new());
        flattener.flat
    }

    pub(crate) fn output(&self) -> Option<(&Path, &FlatNode<'g>)> {
        self.nodes
            .iter()
            .find(|(_, node)| *node.kind == NodeKind::Output && node.scope == Scope::Main)
    }

    pub(crate) fn types(&self, node: &FlatNode<'_>) -> &'g NodeTypes {
        self.report
            .node(&node.scope, node.id)
            .expect("a flattened node is checked")
    }

    pub(crate) fn output_type(&self, path: &Path, port: &str) -> Option<ValueType> {
        let node = self.nodes.get(path)?;
        let types = self.types(node);
        match node.kind {
            NodeKind::FunctionOutput(_) => types.inputs.get("value").copied(),
            _ => types.outputs.get(port).copied(),
        }
    }

    pub(crate) fn spec(&self, node: &FlatNode<'_>) -> Option<&'g NodeSpec> {
        match node.kind {
            NodeKind::Extern(id) => self.library.get(id),
            _ => None,
        }
    }

    pub(crate) fn cone(&self, roots: impl IntoIterator<Item = Path>) -> BTreeSet<Path> {
        let mut seen = BTreeSet::new();
        let mut pending: Vec<Path> = roots.into_iter().collect();
        while let Some(path) = pending.pop() {
            let Some(node) = self.nodes.get(&path) else {
                continue;
            };
            if !seen.insert(path) {
                continue;
            }
            for (_, from, _) in node.sources() {
                pending.push(from.clone());
            }
        }
        seen
    }

    pub(crate) fn order(&self, live: &BTreeSet<Path>) -> Vec<Path> {
        let mut waiting: BTreeMap<&Path, usize> = BTreeMap::new();
        let mut users: BTreeMap<&Path, Vec<&Path>> = BTreeMap::new();
        for path in live {
            let node = &self.nodes[path];
            let mut count = 0;
            for (_, from, _) in node.sources() {
                if live.contains(from) {
                    count += 1;
                    users.entry(from).or_default().push(path);
                }
            }
            waiting.insert(path, count);
        }
        let mut ready: BTreeSet<&Path> = waiting
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(path, _)| *path)
            .collect();
        let mut order = Vec::new();
        while let Some(path) = ready.pop_first() {
            order.push(path.clone());
            for user in users.get(path).into_iter().flatten() {
                let count = waiting.get_mut(user).expect("a live node");
                *count -= 1;
                if *count == 0 {
                    ready.insert(user);
                }
            }
        }
        order
    }

    pub(crate) fn taint(&self, node: &FlatNode<'_>) -> Option<Taint> {
        let own = match node.kind {
            NodeKind::Input(input) => input_taint(*input),
            NodeKind::Op(op) => match op.info().needs {
                Needs::Indirect => Some(Taint::Indirect),
                Needs::Nothing | Needs::Pixel => None,
            },
            NodeKind::Extern(_) => match self.spec(node).map(|spec| &spec.behavior) {
                Some(Behavior::Function(function)) => code_taint(function.code),
                _ => None,
            },
            _ => None,
        };
        own.or_else(|| {
            node.inputs.iter().find_map(|(_, source)| match source {
                Source::Context(input) => input_taint(*input),
                _ => None,
            })
        })
    }

    pub(crate) fn unavailable(&self, node: &FlatNode<'_>, site: Site) -> Option<String> {
        (!available(node.kind, self.library, site)).then(|| node.kind.title(self.library))
    }

    pub(crate) fn site_errors(&self, live: &BTreeSet<Path>, site: Site) -> Vec<GraphError> {
        let mut errors = Vec::new();
        for path in live {
            let node = &self.nodes[path];
            if let Some(what) = self.unavailable(node, site) {
                errors.push(GraphError::new(
                    node.at(None),
                    Problem::Unavailable {
                        what,
                        stage: site.name(),
                    },
                ));
            }
            for (port, source) in &node.inputs {
                if let Source::Context(input) = source
                    && !input_available(*input, site)
                {
                    errors.push(GraphError::new(
                        node.at(Some(port)),
                        Problem::DefaultUnavailable {
                            what: input.title(),
                            stage: site.name(),
                        },
                    ));
                }
            }
        }
        errors
    }
}

pub(crate) fn input_available(input: Input, site: Site) -> bool {
    match site {
        Site::Uniform => input == Input::Time,
        _ => input_code(input, site).is_some(),
    }
}

pub(crate) fn available(kind: &NodeKind, library: &Library, site: Site) -> bool {
    match kind {
        NodeKind::Input(input) => input_available(*input, site),
        NodeKind::Op(op) => match op.info().needs {
            Needs::Nothing => true,
            Needs::Pixel => site.pixel().is_some(),
            Needs::Indirect => site == Site::Indirect,
        },
        NodeKind::Texture(_) => site != Site::Uniform,
        NodeKind::Parameter(_) => site.is_material(),
        NodeKind::Layer(_) | NodeKind::Sample => site == Site::Pixel,
        NodeKind::Lighting
        | NodeKind::Emission
        | NodeKind::Transparent
        | NodeKind::MixShader
        | NodeKind::AddShader
        | NodeKind::ShaderToRgb => matches!(site, Site::Surface | Site::Post),
        NodeKind::Extern(id) => match library.get(id).map(|spec| &spec.behavior) {
            Some(Behavior::Function(function)) => {
                site != Site::Uniform && code_available(function.code, site)
            }
            Some(Behavior::Closure(_)) => matches!(site, Site::Surface | Site::Post),
            Some(Behavior::Filter) => site == Site::Pixel,
            Some(Behavior::Record) => site == Site::Uniform,
            None => true,
        },
        _ => true,
    }
}

struct Flattener<'g, 'r> {
    graph: &'g Graph,
    report: &'r Report,
    stack: Vec<String>,
    flat: Flat<'g>,
}

impl<'g> Flattener<'g, '_> {
    fn inline(
        &mut self,
        scope: &Scope,
        body: &'g Body,
        prefix: &Path,
        bindings: &BTreeMap<String, Source>,
    ) {
        let graph = self.graph;
        for (id, node) in &body.nodes {
            let Some(types) = self.report.node(scope, *id) else {
                continue;
            };
            let mut path = prefix.clone();
            path.push(*id);
            let source = |port: &str| self::source(graph, scope, body, prefix, *id, port, types);
            match &node.kind {
                NodeKind::Call(name) => {
                    if self.stack.contains(name) {
                        continue;
                    }
                    let Some(function) = graph.function(name) else {
                        continue;
                    };
                    let inner: BTreeMap<String, Source> = function
                        .inputs
                        .iter()
                        .map(|port| (port.name.clone(), source(&port.name)))
                        .collect();
                    self.stack.push(name.clone());
                    self.inline(
                        &Scope::Function(name.clone()),
                        &function.body,
                        &path,
                        &inner,
                    );
                    self.stack.pop();
                }
                NodeKind::FunctionInput(name) => {
                    let bound = bindings.get(name).cloned().unwrap_or(Source::Missing);
                    self.flat.nodes.insert(
                        path,
                        FlatNode {
                            kind: &node.kind,
                            scope: scope.clone(),
                            id: *id,
                            inputs: vec![("value".to_owned(), bound)],
                        },
                    );
                }
                _ => {
                    let inputs = types
                        .ports
                        .inputs
                        .iter()
                        .map(|port| (port.name.clone(), source(&port.name)))
                        .collect();
                    self.flat.nodes.insert(
                        path,
                        FlatNode {
                            kind: &node.kind,
                            scope: scope.clone(),
                            id: *id,
                            inputs,
                        },
                    );
                }
            }
        }
    }
}

fn link(graph: &Graph, body: &Body, prefix: &Path, from: &PortRef) -> Source {
    let mut path = prefix.clone();
    path.push(from.node);
    match body.node(from.node).map(|node| &node.kind) {
        Some(NodeKind::Call(name)) => {
            let Some(function) = graph.function(name) else {
                return Source::Missing;
            };
            let set = function.body.nodes.iter().find(
                |(_, node)| matches!(&node.kind, NodeKind::FunctionOutput(output) if *output == from.port),
            );
            match set {
                Some((output, _)) => {
                    path.push(*output);
                    Source::Link(path, "value".to_owned())
                }
                None => function
                    .outputs
                    .iter()
                    .find(|output| output.name == from.port)
                    .and_then(|output| output.default)
                    .map_or(Source::Missing, Source::Constant),
            }
        }
        _ => Source::Link(path, from.port.clone()),
    }
}

fn source(
    graph: &Graph,
    scope: &Scope,
    body: &Body,
    prefix: &Path,
    id: NodeId,
    port: &str,
    types: &NodeTypes,
) -> Source {
    let to = PortRef::new(id, port);
    let declared = types.ports.input(port);
    if declared.is_some_and(|declared| declared.many) {
        let links: Vec<Slot> = body
            .sources(&to)
            .filter_map(|from| match link(graph, body, prefix, from) {
                Source::Link(path, output) => Some((path, output)),
                _ => None,
            })
            .collect();
        return Source::Links(links);
    }
    if let Some(from) = body.source(&to) {
        return link(graph, body, prefix, from);
    }
    let node = body.node(id).expect("a flattened node exists");
    if let Some(held) = node.inputs.get(port) {
        return Source::Socket(SocketRef::new(scope.clone(), id, port), held.clone());
    }
    match declared.map(|declared| declared.default) {
        Some(PortDefault::Value(constant)) => Source::Constant(constant),
        Some(PortDefault::Input(input)) => Source::Context(input),
        Some(PortDefault::Empty) => Source::Empty,
        Some(PortDefault::Required) | None => Source::Missing,
    }
}
