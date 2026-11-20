use rikka_app::prelude::*;
use rikka_shader_graph::NodeKind;
use rikka_widgets::column;
use rikka_widgets::node_canvas::{CanvasMenu, PortSide};

use crate::catalog::{CatalogLine, Dragged, catalog_lines};
use crate::traits::{Desk, Workshop};
use crate::views::editor::at_pointer;
use crate::views::wording;

const KEY: &str = "animate-shader-catalog";
const WIDTH: f32 = 320.0;
const LIST: f32 = 320.0;
const ROW: f32 = 28.0;

fn dragged(studio: &dyn Workshop, menu: &CanvasMenu) -> Option<Dragged> {
    let from = menu.from?;
    let (_, graph) = studio.shown()?;
    let scope = studio.shader().scope();
    let body = graph.scope(&scope)?;
    let node = crate::canvas_data::node_id(body, from.node)?;
    let report = graph.validate(studio.library());
    let ports = crate::canvas_data::ports_of(graph, studio.library(), &report, &scope, node);
    Some(match from.side {
        PortSide::Output => {
            let port = ports.outputs.get(from.index)?;
            Dragged::FromOutput(report.output_type(&scope, node, &port.name))
        }
        PortSide::Input => {
            let port = ports.inputs.get(from.index)?;
            Dragged::FromInput(report.input_type(&scope, node, &port.name))
        }
    })
}

pub(crate) fn lines(studio: &dyn Workshop, menu: &CanvasMenu) -> Vec<CatalogLine> {
    let Some((_, graph)) = studio.shown() else {
        return Vec::new();
    };
    let shader = studio.shader();
    let entries = graph.catalog(studio.library(), &shader.scope());
    catalog_lines(entries, &shader.query.text(), dragged(studio, menu))
}

pub(crate) fn catalog(desk: Desk<'_>, menu: CanvasMenu) -> Element {
    let lines = lines(&*desk, &menu);
    let kinds: Vec<Option<(usize, NodeKind)>> = {
        let mut entry_index = 0;
        lines
            .iter()
            .map(|line| match line {
                CatalogLine::Heading(_) => None,
                CatalogLine::Entry(entry) => {
                    entry_index += 1;
                    Some((entry_index - 1, entry.kind.clone()))
                }
            })
            .collect()
    };
    let entries: Vec<&rikka_shader_graph::Entry> = lines
        .iter()
        .filter_map(|line| match line {
            CatalogLine::Entry(entry) => Some(entry),
            CatalogLine::Heading(_) => None,
        })
        .collect();
    let first = entries.first().map(|entry| entry.kind.clone());
    let picked = desk.shader().catalog_pick.unwrap_or(0);
    let doc = entries
        .get(picked)
        .map(|entry| entry.doc.clone())
        .unwrap_or_default();
    let count = lines.len();
    let shown = lines.clone();
    let list = lazy_column(count, move |slot| match &shown[slot.index] {
        CatalogLine::Heading(category) => container(
            text(wording::category(*category))
                .caption()
                .muted()
                .bold()
                .no_wrap(),
        )
        .padding(Inset::xy(10.0, 6.0))
        .fill_width()
        .any(),
        CatalogLine::Entry(entry) => container(text(entry.title.clone()).no_wrap())
            .padding(Inset::xy(18.0, 4.0))
            .height(ROW)
            .fill_width()
            .any(),
    })
    .reachable({
        let kinds = kinds.clone();
        move |index| kinds.get(index).is_some_and(Option::is_some)
    })
    .lit(true)
    .on_select({
        let kinds = kinds.clone();
        desk.act_with(move |studio, index: usize| {
            if let Some(Some((entry, _))) = kinds.get(index) {
                studio.pick_catalog(Some(*entry));
            }
        })
    })
    .on_activate(desk.act_with(move |studio, index: usize| {
        if let Some(Some((_, kind))) = kinds.get(index) {
            studio.add_catalog_node(kind.clone());
        }
    }))
    .activates_on_press(true)
    .height(LIST)
    .fill_width();
    let search = text_field(&desk.shader().query)
        .placeholder(tr!("animate.shader.search"))
        .control_size(ControlSize::Small)
        .autofocus(true)
        .on_change(desk.act(|studio| studio.pick_catalog(None)))
        .on_submit(desk.act(move |studio| {
            if let Some(kind) = first.clone() {
                studio.add_catalog_node(kind);
            }
        }))
        .fill_width();
    let content = column! {
        search;
        list;
        rule();
        text(doc).caption().muted().fill_width();
    }
    .gap(Space::SNUG)
    .padding(Inset::all(10.0))
    .width(WIDTH);
    popover(content)
        .on_dismiss(desk.act(|studio| studio.close_menu()))
        .build()
        .keyed(KEY)
        .anchored(at_pointer(&menu))
}
