use std::collections::{BTreeMap, BTreeSet};

use rikka_slang::{ParameterKind, Role};

use crate::composite;
use crate::error::{At, GraphError, Problem};
use crate::flat::Flat;
use crate::graph::{Body, Graph, GraphKind, NodeId, PortRef, Scope};
use crate::library::Library;
use crate::material;
use crate::node::{NodeKind, PortDefault, Ports};
use crate::ports::{self, swizzle};
use crate::types::{CoercionError, PortType, SocketValue, ValueType};
use crate::world;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NodeTypes {
    pub ports: Ports,
    pub inputs: BTreeMap<String, ValueType>,
    pub outputs: BTreeMap<String, ValueType>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub(crate) errors: Vec<GraphError>,
    pub(crate) types: BTreeMap<Scope, BTreeMap<NodeId, NodeTypes>>,
}

impl Report {
    pub fn errors(&self) -> &[GraphError] {
        &self.errors
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn ports(&self, scope: &Scope, node: NodeId) -> Option<&Ports> {
        self.node(scope, node).map(|types| &types.ports)
    }

    pub fn input_type(&self, scope: &Scope, node: NodeId, port: &str) -> Option<ValueType> {
        self.node(scope, node)?.inputs.get(port).copied()
    }

    pub fn output_type(&self, scope: &Scope, node: NodeId, port: &str) -> Option<ValueType> {
        self.node(scope, node)?.outputs.get(port).copied()
    }

    pub(crate) fn node(&self, scope: &Scope, node: NodeId) -> Option<&NodeTypes> {
        self.types.get(scope)?.get(&node)
    }

    fn push(&mut self, error: GraphError) {
        if !self.errors.contains(&error) {
            self.errors.push(error);
        }
    }
}

pub(crate) fn validate(graph: &Graph, library: &Library) -> Report {
    let mut report = Report {
        errors: Vec::new(),
        types: BTreeMap::new(),
    };
    parameters(graph, &mut report);
    recursion(graph, &mut report);
    for (scope, body) in graph.scopes() {
        let types = check_body(graph, library, &scope, body, &mut report);
        report.types.insert(scope, types);
    }
    outputs(graph, &mut report);
    layout(graph, &mut report);
    for (name, function) in graph.functions() {
        let scope = Scope::Function(name.clone());
        for port in &function.outputs {
            let set = function.body.nodes.values().any(
                |node| matches!(&node.kind, NodeKind::FunctionOutput(output) if *output == port.name),
            );
            if !set && port.default.is_none() {
                report.push(GraphError::new(
                    At::scope(&scope),
                    Problem::MissingFunctionOutput(port.name.clone()),
                ));
            }
        }
    }
    let flat = Flat::new(graph, library, &report);
    let found = match graph.kind() {
        GraphKind::Material => material::diagnose(graph, library, &flat),
        GraphKind::Composite => composite::diagnose(library, &flat),
        GraphKind::World => world::diagnose(library, &flat),
    };
    for error in found {
        report.push(error);
    }
    report
}

pub(crate) fn valid_name(name: &str) -> bool {
    const RESERVED: &[&str] = &[
        "this",
        "true",
        "false",
        "in",
        "out",
        "inout",
        "return",
        "if",
        "else",
        "for",
        "while",
        "do",
        "break",
        "continue",
        "struct",
        "static",
        "const",
        "uniform",
        "discard",
        "switch",
        "case",
        "default",
        "float",
        "int",
        "uint",
        "bool",
        "half",
        "double",
        "void",
        "import",
        "interface",
        "extension",
        "typealias",
        "let",
        "var",
        "namespace",
        "public",
        "private",
        "internal",
        "override",
    ];
    let mut chars = name.chars();
    let start = chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == '_');
    start
        && chars.all(|rest| rest.is_ascii_lowercase() || rest.is_ascii_digit() || rest == '_')
        && !name.starts_with("rikka")
        && !name.starts_with("__")
        && !RESERVED.contains(&name)
}

fn parameters(graph: &Graph, report: &mut Report) {
    if graph.kind() != GraphKind::Material && !graph.parameters().is_empty() {
        report.push(GraphError::new(At::graph(), Problem::Parameters));
    }
    let mut seen = BTreeSet::new();
    for parameter in graph.parameters() {
        let name = &parameter.name;
        let mut fail = |problem| report.push(GraphError::new(At::graph(), problem));
        if !valid_name(name) {
            fail(Problem::InvalidName(name.clone()));
        }
        if !seen.insert(name.as_str()) {
            fail(Problem::Duplicate(name.clone()));
        }
        if parameter
            .default
            .as_ref()
            .is_some_and(|default| !parameter.literal_fits(default))
            || parameter
                .presets
                .iter()
                .any(|preset| !parameter.literal_fits(&preset.value))
        {
            fail(Problem::DefaultMismatch(name.clone()));
        }
        if parameter
            .role
            .is_some_and(|role| !role.fits(&parameter.kind))
        {
            fail(Problem::RoleMismatch(name.clone()));
        }
        if material::RESERVED.contains(&name.as_str()) || name.starts_with(material::SOCKET_PREFIX)
        {
            fail(Problem::Reserved(name.clone()));
        }
        if parameter
            .texts()
            .any(|text| text.contains(['"', '\\', '|', '\n', '\r']) || text.is_empty())
        {
            fail(Problem::UnsafeText(name.clone()));
        }
    }
}

fn recursion(graph: &Graph, report: &mut Report) {
    fn visit(
        graph: &Graph,
        name: &str,
        stack: &mut Vec<String>,
        done: &mut BTreeSet<String>,
        report: &mut Report,
    ) {
        if done.contains(name) {
            return;
        }
        let Some(function) = graph.function(name) else {
            return;
        };
        stack.push(name.to_owned());
        for (id, node) in &function.body.nodes {
            let NodeKind::Call(callee) = &node.kind else {
                continue;
            };
            if stack.iter().any(|open| open == callee) {
                report.push(GraphError::new(
                    At::node(&Scope::Function(name.to_owned()), *id),
                    Problem::Recursion(callee.clone()),
                ));
            } else {
                visit(graph, callee, stack, done, report);
            }
        }
        stack.pop();
        done.insert(name.to_owned());
    }
    let mut done = BTreeSet::new();
    for name in graph.functions().keys() {
        visit(graph, name, &mut Vec::new(), &mut done, report);
    }
}

fn outputs(graph: &Graph, report: &mut Report) {
    let mut found = None;
    for (id, node) in &graph.body().nodes {
        if node.kind != NodeKind::Output {
            continue;
        }
        if found.replace(*id).is_some() {
            report.push(GraphError::new(
                At::node(&Scope::Main, *id),
                Problem::DuplicateOutput,
            ));
        }
    }
    let Some(output) = found else {
        report.push(GraphError::new(At::graph(), Problem::MissingOutput));
        return;
    };
    if graph.kind() != GraphKind::Material {
        return;
    }
    let displaces = graph.body().nodes[&output]
        .inputs
        .contains_key("displacement")
        || graph
            .body()
            .source(&PortRef::new(output, "displacement"))
            .is_some();
    let bounded = graph.parameters().iter().any(|parameter| {
        parameter.role == Some(Role::Displacement) && parameter.kind == ParameterKind::Float
    });
    if displaces && !bounded {
        report.push(GraphError::new(At::graph(), Problem::Unbounded));
    }
}

fn layout(graph: &Graph, report: &mut Report) {
    let finite = |values: &[f32]| values.iter().all(|value| value.is_finite());
    for (scope, body) in graph.scopes() {
        for (id, node) in &body.nodes {
            if !finite(&node.position) {
                report.push(GraphError::new(
                    At::node(&scope, *id),
                    Problem::InvalidLayout,
                ));
            }
        }
        for group in body.groups.values() {
            if group.color.is_some_and(|color| !finite(&color)) {
                report.push(GraphError::new(At::scope(&scope), Problem::InvalidLayout));
            }
            for node in &group.nodes {
                if !body.nodes.contains_key(node) {
                    report.push(GraphError::new(
                        At::node(&scope, *node),
                        Problem::UnknownNode(*node),
                    ));
                }
            }
        }
        for comment in body.comments.values() {
            let sized = comment.size.iter().all(|side| *side > 0.0);
            if !finite(&comment.position) || !finite(&comment.size) || !sized {
                report.push(GraphError::new(At::scope(&scope), Problem::InvalidLayout));
            }
        }
    }
}

fn held_type(value: &SocketValue) -> ValueType {
    match value {
        SocketValue::Number(constant) => constant.ty(),
        SocketValue::Asset(_) => ValueType::Texture,
    }
}

fn check_body(
    graph: &Graph,
    library: &Library,
    scope: &Scope,
    body: &Body,
    report: &mut Report,
) -> BTreeMap<NodeId, NodeTypes> {
    let mut known: BTreeMap<NodeId, Ports> = BTreeMap::new();
    let mut function_outputs = BTreeSet::new();
    for (id, node) in &body.nodes {
        match ports::ports(graph, library, scope, &node.kind) {
            Ok(ports) => {
                for (port, held) in &node.inputs {
                    let at = At::port(scope, *id, port);
                    let Some(found) = ports.input(port) else {
                        report.push(GraphError::new(at, Problem::UnknownInput(port.clone())));
                        continue;
                    };
                    let fits = match (held, found.ty) {
                        (SocketValue::Number(constant), _) if !constant.is_finite() => {
                            report.push(GraphError::new(at, Problem::NotFinite));
                            continue;
                        }
                        (SocketValue::Number(_), PortType::Dynamic) => true,
                        (SocketValue::Number(_), PortType::Value(ty)) => ty.is_number(),
                        (SocketValue::Asset(_), PortType::Value(ValueType::Texture)) => true,
                        (SocketValue::Asset(_), _) => false,
                    };
                    if !fits {
                        report.push(GraphError::new(
                            at,
                            Problem::Held {
                                found: match held {
                                    SocketValue::Number(_) => "a number",
                                    SocketValue::Asset(_) => "a file",
                                },
                                to: match found.ty {
                                    PortType::Value(ty) => ty,
                                    PortType::Dynamic => ValueType::Float,
                                },
                            },
                        ));
                    }
                }
                known.insert(*id, ports);
            }
            Err(problem) => report.push(GraphError::new(At::node(scope, *id), problem)),
        }
        if let NodeKind::FunctionOutput(name) = &node.kind
            && !function_outputs.insert(name.as_str())
        {
            report.push(GraphError::new(
                At::node(scope, *id),
                Problem::DuplicateFunctionOutput(name.clone()),
            ));
        }
    }

    let mut links: BTreeMap<(NodeId, &str), Vec<&PortRef>> = BTreeMap::new();
    for connection in &body.connections {
        let to = &connection.to;
        let from = &connection.from;
        let at = At::port(scope, to.node, &to.port);
        let target = known.get(&to.node).and_then(|ports| ports.input(&to.port));
        let problem = if !body.nodes.contains_key(&to.node) {
            Some(Problem::UnknownNode(to.node))
        } else if !body.nodes.contains_key(&from.node) {
            Some(Problem::UnknownNode(from.node))
        } else if known.contains_key(&to.node) && target.is_none() {
            Some(Problem::UnknownInput(to.port.clone()))
        } else if known
            .get(&from.node)
            .is_some_and(|ports| ports.output(&from.port).is_none())
        {
            Some(Problem::UnknownOutput(from.port.clone()))
        } else if links.contains_key(&(to.node, to.port.as_str()))
            && !target.is_some_and(|port| port.many)
        {
            Some(Problem::AlreadyConnected)
        } else {
            None
        };
        match problem {
            Some(problem) => report.push(GraphError::new(at, problem)),
            None => links
                .entry((to.node, to.port.as_str()))
                .or_default()
                .push(from),
        }
    }

    let mut waiting: BTreeMap<NodeId, usize> = known.keys().map(|id| (*id, 0)).collect();
    let mut users: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for ((to, _), sources) in &links {
        for from in sources {
            if waiting.contains_key(to) && waiting.contains_key(&from.node) {
                *waiting.get_mut(to).expect("a known node") += 1;
                users.entry(from.node).or_default().push(*to);
            }
        }
    }
    let mut ready: BTreeSet<NodeId> = waiting
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order = Vec::new();
    while let Some(id) = ready.pop_first() {
        order.push(id);
        for user in users.get(&id).into_iter().flatten() {
            let count = waiting.get_mut(user).expect("a known node");
            *count -= 1;
            if *count == 0 {
                ready.insert(*user);
            }
        }
    }
    for id in known.keys() {
        if !order.contains(id) {
            report.push(GraphError::new(At::node(scope, *id), Problem::Cycle));
        }
    }

    let mut types: BTreeMap<NodeId, NodeTypes> = BTreeMap::new();
    for id in order {
        let node = &body.nodes[&id];
        let ports = known.remove(&id).expect("an ordered node has ports");
        let mut sources: Vec<(&str, Vec<ValueType>)> = Vec::new();
        for port in &ports.inputs {
            let found = match links.get(&(id, port.name.as_str())) {
                Some(sources) => sources
                    .iter()
                    .filter_map(|from| {
                        types
                            .get(&from.node)
                            .and_then(|from_types| from_types.outputs.get(&from.port).copied())
                    })
                    .collect(),
                None => match (node.inputs.get(&port.name), port.default) {
                    (Some(held), _) => vec![held_type(held)],
                    (None, PortDefault::Value(constant)) => vec![constant.ty()],
                    (None, PortDefault::Input(input)) => vec![input.ty()],
                    (None, PortDefault::Empty) => Vec::new(),
                    (None, PortDefault::Required) => {
                        report.push(GraphError::new(
                            At::port(scope, id, &port.name),
                            Problem::Required,
                        ));
                        Vec::new()
                    }
                },
            };
            sources.push((&port.name, found));
        }
        let mut width = 1;
        for (port, (name, found)) in ports.inputs.iter().zip(&sources) {
            if port.ty != PortType::Dynamic {
                continue;
            }
            for source in found {
                match (source.width(), source) {
                    (Some(found), _) => width = width.max(found),
                    (None, ValueType::Image) => width = 4,
                    (None, _) => report.push(GraphError::new(
                        At::port(scope, id, name),
                        Problem::Mismatch {
                            from: *source,
                            to: ValueType::Float,
                        },
                    )),
                }
            }
        }
        let resolved = ValueType::vector(width).expect("a vector width");
        let target = |ty: PortType| match ty {
            PortType::Dynamic => resolved,
            PortType::Value(ty) => ty,
        };
        let mut inputs = BTreeMap::new();
        for (port, (name, found)) in ports.inputs.iter().zip(&sources) {
            let to = target(port.ty);
            inputs.insert(port.name.clone(), to);
            for from in found {
                if !from.is_number() && *from != ValueType::Image && port.ty == PortType::Dynamic {
                    continue;
                }
                match from.coerce_to(to) {
                    Ok(_) => {}
                    Err(CoercionError::Widen) => report.push(GraphError::new(
                        At::port(scope, id, name),
                        Problem::Widen { from: *from, to },
                    )),
                    Err(CoercionError::Mismatch) => report.push(GraphError::new(
                        At::port(scope, id, name),
                        Problem::Mismatch { from: *from, to },
                    )),
                }
            }
        }
        if let NodeKind::Swizzle(mask) = &node.kind {
            let chosen = swizzle(mask).expect("a swizzle with ports is valid");
            if let Some(beyond) = chosen.iter().position(|component| *component >= width) {
                report.push(GraphError::new(
                    At::port(scope, id, "value"),
                    Problem::SwizzleRange {
                        component: mask.chars().nth(beyond).expect("a mask character"),
                        ty: resolved,
                    },
                ));
            }
        }
        let outputs = ports
            .outputs
            .iter()
            .map(|port| (port.name.clone(), target(port.ty)))
            .collect();
        types.insert(
            id,
            NodeTypes {
                ports,
                inputs,
                outputs,
            },
        );
    }
    types
}
