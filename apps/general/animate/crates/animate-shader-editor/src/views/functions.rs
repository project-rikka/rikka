use animate_shader::PortSide;
use animate_ui::{glyph, heading};
use rikka_app::prelude::*;
use rikka_shader_graph::{FunctionPort, Graph, GraphKind, ValueType};

use crate::traits::{Desk, Workshop};
use crate::views::wording;

fn port_types(kind: GraphKind) -> Vec<ValueType> {
    let mut types = vec![
        ValueType::Float,
        ValueType::Float2,
        ValueType::Float3,
        ValueType::Float4,
    ];
    match kind {
        GraphKind::Material => types.extend([ValueType::Texture, ValueType::Shader]),
        GraphKind::Composite => types.extend([ValueType::Texture, ValueType::Image]),
        GraphKind::World => {}
    }
    types
}

fn port_line(desk: Desk<'_>, function: &str, side: PortSide, port: &FunctionPort) -> Element {
    let arrow = match side {
        PortSide::Input => MaterialSymbol::LOGIN,
        PortSide::Output => MaterialSymbol::CHEVRON_RIGHT,
    };
    let (function, name) = (function.to_owned(), port.name.clone());
    row_of([
        icon(arrow).extent(Length::px(14.0)).any(),
        text(port.name.clone()).caption().no_wrap().grow(1.0).any(),
        text(wording::value_type(port.ty))
            .caption()
            .muted()
            .no_wrap()
            .any(),
        glyph(MaterialSymbol::CLOSE)
            .ghost()
            .on_press(desk.act(move |studio| {
                studio.edit_graph(None, |graph, library| {
                    animate_shader::remove_function_port(graph, library, &function, side, &name)
                });
            }))
            .tooltip(tr!("animate.shader.remove_port")),
    ])
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn add_port(studio: &mut dyn Workshop, function: &str, side: PortSide, ty: ValueType) {
    let base = match side {
        PortSide::Input => "input",
        PortSide::Output => "output",
    };
    studio.edit_graph(None, |graph, library| {
        animate_shader::add_function_port(graph, library, function, side, base, ty)
    });
}

pub(crate) fn functions(desk: Desk<'_>, graph: &Graph) -> Element {
    let mut parts = vec![
        row_of([
            container(heading(tr!("animate.shader.groups")))
                .grow(1.0)
                .any(),
            glyph(MaterialSymbol::ADD)
                .ghost()
                .on_press(desk.act(|studio| studio.add_function()))
                .tooltip(tr!("animate.shader.add_group")),
        ])
        .align_items(Align::Center)
        .fill_width()
        .any(),
    ];
    if graph.functions().is_empty() {
        parts.push(
            text(tr!("animate.shader.no_groups"))
                .caption()
                .muted()
                .any(),
        );
    }
    let types = port_types(graph.kind());
    let chosen = desk.shader().port_type.min(types.len() - 1);
    for (name, function) in graph.functions() {
        let (entered, removed) = (name.clone(), name.clone());
        let mut rows = vec![
            row_of([
                text(name.clone()).bold().no_wrap().grow(1.0).any(),
                glyph(MaterialSymbol::EDIT)
                    .ghost()
                    .on_press(desk.act(move |studio| studio.enter(&entered)))
                    .tooltip(tr!("animate.shader.enter_group")),
                glyph(MaterialSymbol::DELETE)
                    .ghost()
                    .on_press(desk.act(move |studio| studio.remove_function(&removed)))
                    .tooltip(tr!("animate.shader.remove_group")),
            ])
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width()
            .any(),
        ];
        if !function.doc.is_empty() {
            rows.push(
                text(function.doc.clone())
                    .caption()
                    .muted()
                    .fill_width()
                    .any(),
            );
        }
        for port in &function.inputs {
            rows.push(port_line(desk, name, PortSide::Input, port));
        }
        for port in &function.outputs {
            rows.push(port_line(desk, name, PortSide::Output, port));
        }
        let ty = types[chosen];
        let (inputs, outputs) = (name.clone(), name.clone());
        rows.push(
            row_of([
                combo(Some(chosen))
                    .options(types.iter().map(|ty| wording::value_type(*ty)))
                    .on_select(desk.act_with(|studio, index: usize| {
                        studio.shader_mut().port_type = index;
                    }))
                    .focusless()
                    .control_size(ControlSize::Small)
                    .grow(1.0)
                    .any(),
                button(text(tr!("animate.shader.add_input")).caption())
                    .small()
                    .ghost()
                    .focusless()
                    .on_press(
                        desk.act(move |studio| add_port(studio, &inputs, PortSide::Input, ty)),
                    )
                    .any(),
                button(text(tr!("animate.shader.add_output")).caption())
                    .small()
                    .ghost()
                    .focusless()
                    .on_press(
                        desk.act(move |studio| add_port(studio, &outputs, PortSide::Output, ty)),
                    )
                    .any(),
            ])
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width()
            .any(),
        );
        parts.push(
            container(column_of(rows).gap(Space::TIGHT).fill_width())
                .padding(Inset::all(8.0))
                .background(palette().surface_variant)
                .radius(Corner::Control)
                .fill_width()
                .any(),
        );
    }
    column_of(parts).gap(Space::SNUG).fill_width().any()
}
