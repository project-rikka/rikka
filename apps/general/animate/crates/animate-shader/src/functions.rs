use std::collections::{BTreeMap, BTreeSet};

use rikka_shader_graph::{
    At, Change, Edit, Function, FunctionPort, Graph, GraphError, Library, NodeId, NodeKind,
    PortRef, Problem, Scope, SocketValue, ValueType,
};

use animate_look::fresh_name;

const BORDER: f32 = 260.0;
const SPACING: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortSide {
    Input,
    Output,
}

struct Work<'l> {
    graph: Graph,
    library: &'l Library,
    undo: Vec<Edit>,
}

impl<'l> Work<'l> {
    fn new(graph: &Graph, library: &'l Library) -> Self {
        Self {
            graph: graph.clone(),
            library,
            undo: Vec::new(),
        }
    }

    fn run<T>(
        &mut self,
        step: impl FnOnce(&mut Graph, &Library) -> Result<Change<T>, GraphError>,
    ) -> Result<T, GraphError> {
        let Change { value, undo } = step(&mut self.graph, self.library)?;
        self.undo.push(undo);
        Ok(value)
    }

    fn connect(&mut self, scope: &Scope, from: PortRef, to: PortRef) -> Result<(), GraphError> {
        let scope = scope.clone();
        self.run(|graph, library| graph.edit(library, scope).connect(from, to))
    }

    fn set(
        &mut self,
        scope: &Scope,
        node: NodeId,
        port: &str,
        value: Option<SocketValue>,
    ) -> Result<(), GraphError> {
        let scope = scope.clone();
        self.run(|graph, library| graph.edit(library, scope).set_input(node, port, value))
    }

    fn add(
        &mut self,
        scope: &Scope,
        kind: NodeKind,
        position: [f32; 2],
    ) -> Result<NodeId, GraphError> {
        let scope = scope.clone();
        self.run(|graph, library| graph.edit(library, scope).add_node(kind, position))
    }

    fn remove(&mut self, scope: &Scope, node: NodeId) -> Result<(), GraphError> {
        let scope = scope.clone();
        self.run(|graph, library| graph.edit(library, scope).remove_node(node))
    }

    fn finish<T>(self, into: &mut Graph, value: T) -> Change<T> {
        *into = self.graph;
        let mut undo = self.undo;
        undo.reverse();
        Change {
            value,
            undo: Edit::Batch(undo),
        }
    }
}

fn fail(scope: &Scope, node: Option<NodeId>, port: Option<&str>, problem: Problem) -> GraphError {
    GraphError {
        at: At {
            scope: scope.clone(),
            node,
            port: port.map(str::to_owned),
        },
        problem,
    }
}

fn unknown_function(name: &str) -> GraphError {
    fail(
        &Scope::Main,
        None,
        None,
        Problem::UnknownFunction(name.to_owned()),
    )
}

fn port_name(base: &str, taken: &[FunctionPort]) -> String {
    let mut clean: String = base
        .chars()
        .map(|ch| match ch.is_ascii_alphanumeric() {
            true => ch.to_ascii_lowercase(),
            false => '_',
        })
        .collect();
    if clean.is_empty() || clean.starts_with(|ch: char| ch.is_ascii_digit()) {
        clean.insert_str(0, "port_");
    }
    (1..)
        .map(|number| match number {
            1 => clean.clone(),
            _ => format!("{clean}_{number}"),
        })
        .find(|name| taken.iter().all(|port| port.name != *name))
        .expect("some numbered name is free")
}

pub fn fresh_function(graph: &Graph, base: &str) -> String {
    fresh_name(base, |name| graph.function(name).is_some())
}

struct Crossing {
    port: String,
    ty: ValueType,
    ends: Vec<PortRef>,
}

struct Crossings {
    inputs: BTreeMap<PortRef, Crossing>,
    outputs: BTreeMap<PortRef, Crossing>,
}

fn crossings(
    graph: &Graph,
    library: &Library,
    scope: &Scope,
    nodes: &BTreeSet<NodeId>,
) -> Result<Crossings, GraphError> {
    let body = graph
        .scope(scope)
        .ok_or_else(|| fail(scope, None, None, Problem::UnknownFunction(String::new())))?;
    let report = graph.validate(library);
    let typed = |from: &PortRef| {
        report
            .output_type(scope, from.node, &from.port)
            .ok_or_else(|| {
                fail(
                    scope,
                    Some(from.node),
                    Some(&from.port),
                    Problem::UnknownOutput(from.port.clone()),
                )
            })
    };
    let mut inputs: BTreeMap<PortRef, Crossing> = BTreeMap::new();
    let mut outputs: BTreeMap<PortRef, Crossing> = BTreeMap::new();
    let mut input_ports: Vec<FunctionPort> = Vec::new();
    let mut output_ports: Vec<FunctionPort> = Vec::new();
    for connection in body.connections() {
        let inside = (
            nodes.contains(&connection.from.node),
            nodes.contains(&connection.to.node),
        );
        let (map, ports, base, key, end) = match inside {
            (false, true) => (
                &mut inputs,
                &mut input_ports,
                &connection.to.port,
                &connection.from,
                &connection.to,
            ),
            (true, false) => (
                &mut outputs,
                &mut output_ports,
                &connection.from.port,
                &connection.from,
                &connection.to,
            ),
            _ => continue,
        };
        if let Some(crossing) = map.get_mut(key) {
            crossing.ends.push(end.clone());
            continue;
        }
        let ty = typed(key)?;
        let port = port_name(base, ports);
        ports.push(FunctionPort::new(port.clone(), ty));
        map.insert(
            key.clone(),
            Crossing {
                port,
                ty,
                ends: vec![end.clone()],
            },
        );
    }
    Ok(Crossings { inputs, outputs })
}

pub fn collapse(
    graph: &mut Graph,
    library: &Library,
    scope: &Scope,
    nodes: &BTreeSet<NodeId>,
    name: &str,
) -> Result<Change<NodeId>, GraphError> {
    let Crossings { inputs, outputs } = crossings(graph, library, scope, nodes)?;
    let body = graph
        .scope(scope)
        .ok_or_else(|| fail(scope, None, None, Problem::UnknownFunction(String::new())))?;
    let positions: Vec<[f32; 2]> = nodes
        .iter()
        .filter_map(|id| body.node(*id).map(|node| node.position))
        .collect();
    let left = positions
        .iter()
        .map(|at| at[0])
        .fold(f32::INFINITY, f32::min);
    let right = positions
        .iter()
        .map(|at| at[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let top = positions
        .iter()
        .map(|at| at[1])
        .fold(f32::INFINITY, f32::min);
    let middle = positions
        .iter()
        .fold([0.0, 0.0], |sum, at| [sum[0] + at[0], sum[1] + at[1]]);
    let count = positions.len().max(1) as f32;
    let middle = [middle[0] / count, middle[1] / count];
    let function = Function {
        inputs: inputs
            .values()
            .map(|crossing| FunctionPort::new(crossing.port.clone(), crossing.ty))
            .collect(),
        outputs: outputs
            .values()
            .map(|crossing| FunctionPort::new(crossing.port.clone(), crossing.ty))
            .collect(),
        ..Function::default()
    };
    let inner = Scope::Function(name.to_owned());
    let mut work = Work::new(graph, library);
    work.run(|graph, library| graph.add_function(library, name, function))?;
    let fragment = work.graph.copy(scope, nodes)?;
    let moved = {
        let inner = inner.clone();
        work.run(|graph, library| graph.edit(library, inner).paste(&fragment, [0.0, 0.0]))?
    };
    for (row, crossing) in inputs.values().enumerate() {
        let port = work.add(
            &inner,
            NodeKind::FunctionInput(crossing.port.clone()),
            [left - BORDER, top + row as f32 * SPACING],
        )?;
        for end in &crossing.ends {
            work.connect(
                &inner,
                PortRef::new(port, "value"),
                PortRef::new(moved[&end.node], end.port.clone()),
            )?;
        }
    }
    for (row, (from, crossing)) in outputs.iter().enumerate() {
        let port = work.add(
            &inner,
            NodeKind::FunctionOutput(crossing.port.clone()),
            [right + BORDER, top + row as f32 * SPACING],
        )?;
        work.connect(
            &inner,
            PortRef::new(moved[&from.node], from.port.clone()),
            PortRef::new(port, "value"),
        )?;
    }
    for id in nodes {
        work.remove(scope, *id)?;
    }
    let call = work.add(scope, NodeKind::Call(name.to_owned()), middle)?;
    for (from, crossing) in &inputs {
        work.connect(
            scope,
            from.clone(),
            PortRef::new(call, crossing.port.clone()),
        )?;
    }
    for crossing in outputs.values() {
        for end in &crossing.ends {
            work.connect(
                scope,
                PortRef::new(call, crossing.port.clone()),
                end.clone(),
            )?;
        }
    }
    Ok(work.finish(graph, call))
}

pub fn expand(
    graph: &mut Graph,
    library: &Library,
    scope: &Scope,
    call: NodeId,
) -> Result<Change<BTreeSet<NodeId>>, GraphError> {
    let body = graph
        .scope(scope)
        .ok_or_else(|| fail(scope, None, None, Problem::UnknownFunction(String::new())))?;
    let node = body
        .node(call)
        .ok_or_else(|| fail(scope, Some(call), None, Problem::UnknownNode(call)))?;
    let NodeKind::Call(name) = &node.kind else {
        return Err(fail(
            scope,
            Some(call),
            None,
            Problem::UnknownFunction(String::new()),
        ));
    };
    let function = graph.function(name).ok_or_else(|| unknown_function(name))?;
    let inner = Scope::Function(name.clone());
    let regular: BTreeSet<NodeId> = function
        .body
        .nodes()
        .iter()
        .filter(|(_, node)| {
            !matches!(
                node.kind,
                NodeKind::FunctionInput(_) | NodeKind::FunctionOutput(_)
            )
        })
        .map(|(id, _)| *id)
        .collect();
    let centre = {
        let positions: Vec<[f32; 2]> = regular
            .iter()
            .filter_map(|id| function.body.node(*id).map(|node| node.position))
            .collect();
        let count = positions.len().max(1) as f32;
        let sum = positions
            .iter()
            .fold([0.0, 0.0], |sum, at| [sum[0] + at[0], sum[1] + at[1]]);
        [sum[0] / count, sum[1] / count]
    };
    let offset = [node.position[0] - centre[0], node.position[1] - centre[1]];
    let fed =
        |port: &str| -> Vec<PortRef> { body.sources(&PortRef::new(call, port)).cloned().collect() };
    let held = |port: &str| -> Option<SocketValue> {
        node.inputs.get(port).cloned().or_else(|| {
            function
                .inputs
                .iter()
                .find(|known| known.name == port)
                .and_then(|known| known.default)
                .map(SocketValue::Number)
        })
    };
    let taken: Vec<(String, Vec<PortRef>)> = function
        .outputs
        .iter()
        .map(|port| {
            let ends = body
                .connections()
                .iter()
                .filter(|connection| connection.from == PortRef::new(call, port.name.clone()))
                .map(|connection| connection.to.clone())
                .collect();
            (port.name.clone(), ends)
        })
        .collect();
    let fragment = graph.copy(&inner, &regular)?;
    let mut work = Work::new(graph, library);
    let moved = {
        let scope = scope.clone();
        work.run(|graph, library| graph.edit(library, scope).paste(&fragment, offset))?
    };
    work.remove(scope, call)?;
    for connection in function.body.connections() {
        let Some(NodeKind::FunctionInput(port)) = function
            .body
            .node(connection.from.node)
            .map(|node| &node.kind)
        else {
            continue;
        };
        let Some(target) = moved.get(&connection.to.node) else {
            continue;
        };
        let sources = fed(port);
        match sources.is_empty() {
            false => {
                for source in sources {
                    work.connect(
                        scope,
                        source,
                        PortRef::new(*target, connection.to.port.clone()),
                    )?;
                }
            }
            true => {
                if let Some(value) = held(port) {
                    work.set(scope, *target, &connection.to.port, Some(value))?;
                }
            }
        }
    }
    for (id, inner_node) in function.body.nodes() {
        let NodeKind::FunctionOutput(port) = &inner_node.kind else {
            continue;
        };
        let Some((_, ends)) = taken.iter().find(|(name, _)| name == port) else {
            continue;
        };
        let source = function.body.source(&PortRef::new(*id, "value")).cloned();
        for end in ends {
            match &source {
                Some(from) => match function.body.node(from.node).map(|node| &node.kind) {
                    Some(NodeKind::FunctionInput(passed)) => {
                        let sources = fed(passed);
                        match sources.is_empty() {
                            false => {
                                for source in sources {
                                    work.connect(scope, source, end.clone())?;
                                }
                            }
                            true => {
                                if let Some(value) = held(passed) {
                                    work.set(scope, end.node, &end.port, Some(value))?;
                                }
                            }
                        }
                    }
                    _ => {
                        let Some(inside) = moved.get(&from.node) else {
                            continue;
                        };
                        work.connect(scope, PortRef::new(*inside, from.port.clone()), end.clone())?;
                    }
                },
                None => {
                    let value = inner_node.inputs.get("value").cloned().or_else(|| {
                        function
                            .outputs
                            .iter()
                            .find(|known| known.name == *port)
                            .and_then(|known| known.default)
                            .map(SocketValue::Number)
                    });
                    if let Some(value) = value {
                        work.set(scope, end.node, &end.port, Some(value))?;
                    }
                }
            }
        }
    }
    let added = moved.values().copied().collect();
    Ok(work.finish(graph, added))
}

pub fn rename_function(
    graph: &mut Graph,
    library: &Library,
    from: &str,
    to: &str,
) -> Result<Change<()>, GraphError> {
    let function = graph
        .function(from)
        .cloned()
        .ok_or_else(|| unknown_function(from))?;
    let old = Scope::Function(from.to_owned());
    let calls = calls_of(graph, from, &old);
    let mut work = Work::new(graph, library);
    work.run(|graph, library| graph.add_function(library, to, function))?;
    for (scope, id) in calls {
        work.run(|graph, library| {
            graph
                .edit(library, scope)
                .set_kind(id, NodeKind::Call(to.to_owned()))
        })?;
    }
    work.run(|graph, library| graph.remove_function(library, from))?;
    Ok(work.finish(graph, ()))
}

pub fn remove_function(
    graph: &mut Graph,
    library: &Library,
    name: &str,
) -> Result<Change<()>, GraphError> {
    if graph.function(name).is_none() {
        return Err(unknown_function(name));
    }
    let own = Scope::Function(name.to_owned());
    let calls = calls_of(graph, name, &own);
    let mut work = Work::new(graph, library);
    for (scope, id) in calls {
        work.remove(&scope, id)?;
    }
    work.run(|graph, library| graph.remove_function(library, name))?;
    Ok(work.finish(graph, ()))
}

pub fn add_function_port(
    graph: &mut Graph,
    library: &Library,
    name: &str,
    side: PortSide,
    base: &str,
    ty: ValueType,
) -> Result<Change<String>, GraphError> {
    let mut function = graph
        .function(name)
        .cloned()
        .ok_or_else(|| unknown_function(name))?;
    let ports = match side {
        PortSide::Input => &mut function.inputs,
        PortSide::Output => &mut function.outputs,
    };
    let port = port_name(base, ports);
    ports.push(FunctionPort::new(port.clone(), ty));
    let change = graph.set_function(library, name, function)?;
    Ok(Change {
        value: port,
        undo: change.undo,
    })
}

pub fn remove_function_port(
    graph: &mut Graph,
    library: &Library,
    name: &str,
    side: PortSide,
    port: &str,
) -> Result<Change<()>, GraphError> {
    let mut function = graph
        .function(name)
        .cloned()
        .ok_or_else(|| unknown_function(name))?;
    let own = Scope::Function(name.to_owned());
    let markers: Vec<NodeId> = function
        .body
        .nodes()
        .iter()
        .filter(|(_, node)| match (&node.kind, side) {
            (NodeKind::FunctionInput(known), PortSide::Input)
            | (NodeKind::FunctionOutput(known), PortSide::Output) => known == port,
            _ => false,
        })
        .map(|(id, _)| *id)
        .collect();
    let calls = calls_of(graph, name, &own);
    let mut work = Work::new(graph, library);
    for id in markers {
        work.remove(&own, id)?;
    }
    for (scope, call) in calls {
        let Some(body) = work.graph.scope(&scope) else {
            continue;
        };
        let attached: Vec<rikka_shader_graph::Connection> = body
            .connections()
            .iter()
            .filter(|connection| match side {
                PortSide::Input => connection.to == PortRef::new(call, port),
                PortSide::Output => connection.from == PortRef::new(call, port),
            })
            .cloned()
            .collect();
        let holds = side == PortSide::Input
            && body
                .node(call)
                .is_some_and(|node| node.inputs.contains_key(port));
        for connection in attached {
            let scope = scope.clone();
            work.run(|graph, library| {
                graph
                    .edit(library, scope)
                    .detach(connection.from, connection.to)
            })?;
        }
        if holds {
            work.set(&scope, call, port, None)?;
        }
    }
    match side {
        PortSide::Input => function.inputs.retain(|known| known.name != port),
        PortSide::Output => function.outputs.retain(|known| known.name != port),
    }
    function.body = work
        .graph
        .function(name)
        .map(|current| current.body.clone())
        .unwrap_or_default();
    work.run(|graph, library| graph.set_function(library, name, function))?;
    Ok(work.finish(graph, ()))
}

fn calls_of(graph: &Graph, name: &str, skip: &Scope) -> Vec<(Scope, NodeId)> {
    graph
        .scopes()
        .filter(|(scope, _)| scope != skip)
        .flat_map(|(scope, body)| {
            body.nodes()
                .iter()
                .filter(|(_, node)| matches!(&node.kind, NodeKind::Call(called) if called == name))
                .map(|(id, _)| (scope.clone(), *id))
                .collect::<Vec<_>>()
        })
        .collect()
}
