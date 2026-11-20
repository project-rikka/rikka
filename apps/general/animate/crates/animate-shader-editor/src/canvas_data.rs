use std::collections::BTreeMap;
use std::path::Path;

use animate_shader::Diagnostic;
use rikka_renderer_3d::Literal;
use rikka_shader_graph::{
    Body, Category, CommentId, Constant, Graph, GroupId, Library, Node, NodeId, NodeKind, PortType,
    Ports, Report, Scope, SocketValue, ValueType,
};
use rikka_types::{Color, Point, Size};
use rikka_widgets::node_canvas::{
    CanvasFrame, CanvasLink, CanvasNode, CanvasPort, FrameKey, NodeGraph, NodeKey, PortAddr,
    PortSide,
};

const COMMENT_KEYS: u64 = 1 << 32;

pub fn port_color(ty: Option<ValueType>) -> Color {
    match ty {
        Some(ValueType::Float) => Color::rgb(0.62, 0.64, 0.7),
        Some(ValueType::Float2) => Color::rgb(0.4, 0.78, 0.5),
        Some(ValueType::Float3) => Color::rgb(0.95, 0.78, 0.3),
        Some(ValueType::Float4) => Color::rgb(0.82, 0.48, 0.9),
        Some(ValueType::Texture) => Color::rgb(0.95, 0.52, 0.3),
        Some(ValueType::Shader) => Color::rgb(0.35, 0.8, 0.45),
        Some(ValueType::Image) => Color::rgb(0.95, 0.62, 0.62),
        Some(ValueType::Record(_)) => Color::rgb(0.45, 0.75, 0.9),
        None => Color::rgb(0.45, 0.6, 0.85),
    }
}

pub fn category_color(category: Category) -> Color {
    match category {
        Category::Math | Category::Trigonometry | Category::Interpolation => {
            Color::rgb(0.36, 0.45, 0.62)
        }
        Category::Vector | Category::Uv => Color::rgb(0.3, 0.55, 0.5),
        Category::Noise => Color::rgb(0.5, 0.42, 0.62),
        Category::Surface | Category::Lighting => Color::rgb(0.72, 0.55, 0.25),
        Category::Color | Category::Ramp => Color::rgb(0.7, 0.4, 0.45),
        Category::Texture => Color::rgb(0.78, 0.45, 0.22),
        Category::Shader | Category::Toon => Color::rgb(0.3, 0.62, 0.38),
        Category::Layer | Category::Filter | Category::Lens | Category::Stylize => {
            Color::rgb(0.3, 0.5, 0.75)
        }
        Category::Environment | Category::Light | Category::Render | Category::Stage => {
            Color::rgb(0.62, 0.52, 0.3)
        }
        Category::Input => Color::rgb(0.35, 0.58, 0.35),
        Category::Parameter => Color::rgb(0.6, 0.35, 0.65),
        Category::Constant => Color::rgb(0.45, 0.45, 0.5),
        Category::Function => Color::rgb(0.55, 0.5, 0.3),
        Category::Output => Color::rgb(0.75, 0.3, 0.3),
    }
}

pub fn node_key(id: NodeId) -> NodeKey {
    NodeKey(u64::from(id.get()))
}

pub fn node_id(body: &Body, key: NodeKey) -> Option<NodeId> {
    body.nodes()
        .keys()
        .find(|id| u64::from(id.get()) == key.0)
        .copied()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameId {
    Group(GroupId),
    Comment(CommentId),
}

pub fn frame_key(id: FrameId) -> FrameKey {
    match id {
        FrameId::Group(id) => FrameKey(u64::from(id.get())),
        FrameId::Comment(id) => FrameKey(u64::from(id.get()) + COMMENT_KEYS),
    }
}

pub fn frame_id(body: &Body, key: FrameKey) -> Option<FrameId> {
    match key.0 >= COMMENT_KEYS {
        true => body
            .comments()
            .keys()
            .find(|id| u64::from(id.get()) + COMMENT_KEYS == key.0)
            .map(|id| FrameId::Comment(*id)),
        false => body
            .groups()
            .keys()
            .find(|id| u64::from(id.get()) == key.0)
            .map(|id| FrameId::Group(*id)),
    }
}

pub(crate) fn ports_of(
    graph: &Graph,
    library: &Library,
    report: &Report,
    scope: &Scope,
    id: NodeId,
) -> Ports {
    if let Some(ports) = report.ports(scope, id) {
        return ports.clone();
    }
    graph
        .scope(scope)
        .and_then(|body| body.node(id))
        .and_then(|node| graph.ports(library, scope, &node.kind).ok())
        .unwrap_or_default()
}

pub(crate) fn port_index(ports: &Ports, side: PortSide, name: &str) -> Option<usize> {
    let list = match side {
        PortSide::Input => &ports.inputs,
        PortSide::Output => &ports.outputs,
    };
    list.iter().position(|port| port.name == name)
}

pub(crate) fn numbers(constant: &Constant) -> String {
    constant
        .values()
        .iter()
        .map(|value| format!("{}", (value * 1000.0).round() / 1000.0))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn shown_value(value: &SocketValue) -> String {
    match value {
        SocketValue::Number(constant) => numbers(constant),
        SocketValue::Asset(path) => Path::new(path)
            .file_name()
            .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned()),
    }
}

fn swatch(values: &[f32]) -> Option<Color> {
    (values.len() >= 3).then(|| {
        Color::rgba(
            values[0].clamp(0.0, 1.0),
            values[1].clamp(0.0, 1.0),
            values[2].clamp(0.0, 1.0),
            values.get(3).copied().unwrap_or(1.0).clamp(0.0, 1.0),
        )
    })
}

fn detail(graph: &Graph, kind: &NodeKind) -> (Option<String>, Option<Color>) {
    match kind {
        NodeKind::Constant(constant) => (Some(numbers(constant)), swatch(constant.values())),
        NodeKind::Parameter(name) => {
            let default = graph
                .parameter(name)
                .and_then(|parameter| parameter.default);
            let swatch = default.and_then(|literal| match literal {
                Literal::Vec3(values) => swatch(&values),
                Literal::Vec4(values) => swatch(&values),
                _ => None,
            });
            (None, swatch)
        }
        NodeKind::Swizzle(mask) => (Some(mask.clone()), None),
        NodeKind::ColorRamp(ramp) => (
            None,
            ramp.stops.first().and_then(|stop| swatch(&stop.color)),
        ),
        _ => (None, None),
    }
}

fn problem(
    report: &Report,
    diagnostics: &[Diagnostic],
    scope: &Scope,
    id: NodeId,
) -> Option<String> {
    report
        .errors()
        .iter()
        .filter(|error| error.at.scope == *scope && error.at.node == Some(id))
        .map(|error| match &error.at.port {
            Some(port) => format!("{port}: {}", error.problem),
            None => error.problem.to_string(),
        })
        .chain(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.node() == Some((scope, id)))
                .map(|diagnostic| diagnostic.message.clone()),
        )
        .next()
}

fn canvas_node(
    graph: &Graph,
    library: &Library,
    report: &Report,
    scope: &Scope,
    id: NodeId,
    node: &Node,
    ports: &Ports,
) -> CanvasNode {
    let (detail, swatch) = detail(graph, &node.kind);
    let typed = |found: Option<ValueType>, ty: PortType| {
        found.or(match ty {
            PortType::Value(ty) => Some(ty),
            PortType::Dynamic => None,
        })
    };
    let mut canvas = CanvasNode::new(
        node_key(id),
        node.kind.title(library),
        Point::new(node.position[0], node.position[1]),
    )
    .accent(category_color(node.kind.category(library)));
    for port in &ports.outputs {
        let ty = typed(report.output_type(scope, id, &port.name), port.ty);
        canvas = canvas.output(CanvasPort::new(port.name.clone(), port_color(ty)));
    }
    for port in &ports.inputs {
        let ty = typed(report.input_type(scope, id, &port.name), port.ty);
        let label = match node.inputs.get(&port.name) {
            Some(value) => format!("{} = {}", port.name, shown_value(value)),
            None => port.name.clone(),
        };
        let shown = CanvasPort::new(label, port_color(ty));
        canvas = canvas.input(match port.many {
            true => shown.many(),
            false => shown,
        });
    }
    if let Some(detail) = detail {
        canvas = canvas.detail(detail);
    }
    if let Some(swatch) = swatch {
        canvas = canvas.swatch(swatch);
    }
    canvas
}

pub fn node_graph(
    graph: &Graph,
    library: &Library,
    scope: &Scope,
    diagnostics: &[Diagnostic],
) -> Option<NodeGraph> {
    let body = graph.scope(scope)?;
    let report = graph.validate(library);
    let mut nodes = Vec::with_capacity(body.nodes().len());
    let mut known: BTreeMap<NodeId, Ports> = BTreeMap::new();
    for (id, node) in body.nodes() {
        let ports = ports_of(graph, library, &report, scope, *id);
        let mut canvas = canvas_node(graph, library, &report, scope, *id, node, &ports);
        if let Some(problem) = problem(&report, diagnostics, scope, *id) {
            canvas = canvas.problem(problem);
        }
        nodes.push(canvas);
        known.insert(*id, ports);
    }
    let index = |id: NodeId, port: &str, side: PortSide| port_index(known.get(&id)?, side, port);
    let links = body
        .connections()
        .iter()
        .filter_map(|connection| {
            Some(CanvasLink::new(
                PortAddr::output(
                    node_key(connection.from.node),
                    index(
                        connection.from.node,
                        &connection.from.port,
                        PortSide::Output,
                    )?,
                ),
                PortAddr::input(
                    node_key(connection.to.node),
                    index(connection.to.node, &connection.to.port, PortSide::Input)?,
                ),
            ))
        })
        .collect();
    let frames = body
        .groups()
        .iter()
        .map(|(id, group)| {
            let frame = CanvasFrame::group(
                frame_key(FrameId::Group(*id)),
                group.title.clone(),
                group.nodes.iter().map(|node| node_key(*node)).collect(),
            );
            match group.color {
                Some([r, g, b, a]) => frame.color(Color::rgba(r, g, b, a)),
                None => frame,
            }
        })
        .chain(body.comments().iter().map(|(id, comment)| {
            CanvasFrame::comment(
                frame_key(FrameId::Comment(*id)),
                comment.text.clone(),
                Point::new(comment.position[0], comment.position[1]),
                Size::new(comment.size[0], comment.size[1]),
            )
        }))
        .collect();
    Some(NodeGraph {
        nodes,
        links,
        frames,
    })
}
