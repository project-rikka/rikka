use animate_look::{GraphRef, SocketKey, Target};
use animate_ui::{glyph, heading, labelled};
use rikka_app::prelude::*;
use rikka_shader_graph::{Graph, NodeId, PortRef, Scope, SocketRef};
use rikka_widgets::node_canvas::{FrameKey, Selection};

use crate::canvas_data::{FrameId, frame_id};
use crate::sockets::{Fallback, Field, SocketRow, SocketState, socket_rows};
use crate::traits::Desk;
use crate::views::rows::{ValueRow, value_row};
use crate::views::{functions, instances, parameters, payload};

const PADDING: f32 = 12.0;

pub fn node_properties(desk: Desk<'_>) -> impl View {
    let body = match desk.shown() {
        None => text(tr!("animate.shader.no_materials"))
            .caption()
            .muted()
            .any(),
        Some((at, graph)) => {
            let shader = desk.shader();
            let scope = shader.scope();
            let nodes: Vec<NodeId> = desk.selected_nodes().into_iter().collect();
            match (nodes.as_slice(), shader.selection().frames().next()) {
                ([id], _) => node_section(desk, &at, graph, &scope, *id),
                ([], Some(key)) => frame_section(desk, graph, &scope, key),
                ([], None) => graph_section(desk, &at, graph),
                (many, _) => text(tr!("animate.shader.many_selected", count = many.len()))
                    .caption()
                    .muted()
                    .any(),
            }
        }
    };
    scrollable(container(body).padding(Inset::all(PADDING)).fill_width()).fill()
}

fn node_section(
    desk: Desk<'_>,
    at: &GraphRef,
    graph: &Graph,
    scope: &Scope,
    id: NodeId,
) -> Element {
    let library = desk.library();
    let Some(node) = graph.scope(scope).and_then(|body| body.node(id)) else {
        return column_of(Vec::<Element>::new()).any();
    };
    let mut parts = vec![
        heading(Label::from(node.kind.title(library))),
        text(node.kind.doc(library))
            .caption()
            .muted()
            .fill_width()
            .any(),
    ];
    parts.extend(payload::kind_editor(
        desk, graph, library, scope, id, &node.kind,
    ));
    let rows = socket_rows(graph, library, scope, id);
    if !rows.is_empty() {
        parts.push(rule().any());
        parts.push(heading(tr!("animate.shader.inputs")));
    }
    for row in rows {
        parts.push(socket(desk, at, scope, id, row));
    }
    parts.push(rule().any());
    let removed = scope.clone();
    parts.push(
        button(text(tr!("animate.shader.delete")).caption())
            .ghost()
            .small()
            .focusless()
            .on_press(desk.act(move |studio| {
                studio.edit_graph(None, |graph, library| {
                    graph.edit(library, removed.clone()).remove_node(id)
                });
                studio.select(Selection::default());
            }))
            .any(),
    );
    column_of(parts).gap(Space::SNUG).fill_width().any()
}

fn socket(desk: Desk<'_>, at: &GraphRef, scope: &Scope, id: NodeId, row: SocketRow) -> Element {
    let label = Label::from(row.port.clone());
    match &row.state {
        SocketState::Linked(sources) => {
            let lines = sources.iter().map(|source| {
                text(format!("{} · {}", source.title, source.port))
                    .caption()
                    .no_wrap()
                    .any()
            });
            let detached: Vec<(PortRef, PortRef)> = sources
                .iter()
                .map(|source| {
                    (
                        PortRef::new(source.node, source.port.clone()),
                        PortRef::new(id, row.port.clone()),
                    )
                })
                .collect();
            let disconnect = glyph(MaterialSymbol::LINK_OFF)
                .ghost()
                .on_press(desk.act(move |studio| {
                    for (from, to) in &detached {
                        studio.detach(from.clone(), to.clone());
                    }
                }))
                .tooltip(tr!("animate.shader.disconnect"));
            labelled(
                label,
                column_of(lines).gap(Space::NONE).fill_width(),
                Some(disconnect),
            )
        }
        SocketState::Default(Fallback::Context(input)) => labelled(
            label,
            text(tr!("animate.shader.from_context", input = &input.title()))
                .caption()
                .muted(),
            None,
        ),
        SocketState::Default(Fallback::Required) => labelled(
            label,
            text(tr!("animate.shader.needs_link"))
                .caption()
                .color(palette().danger),
            None,
        ),
        _ if row.field == Field::Link => labelled(
            label,
            text(tr!("animate.shader.link_only")).caption().muted(),
            None,
        ),
        state => {
            let key = Target::Socket(SocketKey::new(
                at.clone(),
                SocketRef::new(scope.clone(), id, row.port.clone()),
            ));
            let held = desk.value(&key);
            let stored = match state {
                SocketState::Set(value) => Some(value.clone()),
                _ => None,
            };
            let keying = desk.key_state(&key);
            let value = held
                .clone()
                .or(stored.clone())
                .or_else(|| row.fallback_value());
            let document = (row.field == Field::Text).then(|| {
                let current = value
                    .as_ref()
                    .and_then(|value| value.asset())
                    .unwrap_or_default();
                desk.shader().field(&format!("{key:?}"), current)
            });
            value_row(
                desk.reach(),
                ValueRow {
                    set: held.is_some() || stored.is_some(),
                    value,
                    label,
                    field: row.field.clone(),
                    key,
                    document,
                },
                keying,
            )
        }
    }
}

fn frame_section(desk: Desk<'_>, graph: &Graph, scope: &Scope, key: FrameKey) -> Element {
    let title = graph
        .scope(scope)
        .and_then(|body| match frame_id(body, key)? {
            FrameId::Group(id) => body.groups().get(&id).map(|group| group.title.clone()),
            FrameId::Comment(id) => body.comments().get(&id).map(|comment| comment.text.clone()),
        })
        .unwrap_or_default();
    let field = format!("{:?}/{scope:?}/frame/{}", desk.shader().graph(), key.0);
    let document = desk.shader().field(&field, &title);
    let reading = document.clone();
    column_of([
        heading(tr!("animate.shader.frame")),
        text_editor(&document)
            .gutter(false)
            .wrap(true)
            .fits_text(48.0..=200.0)
            .on_blur(desk.act(move |studio| {
                studio.rename_frame(key, reading.text());
                reading.mark_saved();
            }))
            .fill_width()
            .any(),
    ])
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn graph_section(desk: Desk<'_>, at: &GraphRef, graph: &Graph) -> Element {
    let body = graph.body();
    let mut parts = vec![
        text(tr!(
            "animate.shader.summary",
            nodes = body.nodes().len(),
            links = body.connections().len()
        ))
        .caption()
        .muted()
        .any(),
    ];
    match at {
        GraphRef::Material(key) => {
            if let Some(instances) = instances::instances(desk, key, graph) {
                parts.push(instances);
                parts.push(rule().any());
            }
            parts.push(parameters::parameters(desk, graph));
        }
        GraphRef::Composite => {
            if let Ok(plan) = desk.workbench().composite_plan(desk.look()) {
                parts.push(
                    text(tr!("animate.shader.passes", count = plan.steps.len()))
                        .caption()
                        .muted()
                        .any(),
                );
            }
        }
        GraphRef::World => {}
    }
    parts.push(rule().any());
    parts.push(functions::functions(desk, graph));
    column_of(parts).gap(Space::SNUG).fill_width().any()
}
