use std::rc::Rc;

use paint_document::{Layer, LayerId, LayerKind, Placement};
use rikka_app::prelude::*;
use rikka_ui::ImageSource;
use rikka_widgets::{column, row};

use super::grab::{Shape, grab};
use crate::command::Command;
use crate::paint::Paint;
use crate::session::Kind;
use crate::tab::{Layering, Tab};
use crate::thumbnail::thumbnail;
use crate::wording;

const ROW: f32 = 50.0;
const THUMBNAIL: f32 = 40.0;
const INDENT: f32 = 16.0;
const TOGGLE: f32 = 28.0;
const ACTIVE_TINT: f32 = 0.16;
const GLYPH: f32 = 20.0;
const MARK: f32 = 2.0;
const MENU: f32 = 240.0;
const ROW_KEY: &str = "paint-layer-row";

const FOOTER: [(Command, MaterialSymbol, &str); 7] = [
    (
        Command::Layer(Layering::New),
        MaterialSymbol::ADD,
        "paint.command.new_layer",
    ),
    (
        Command::Layer(Layering::Duplicate),
        MaterialSymbol::CONTENT_COPY,
        "paint.command.duplicate_layer",
    ),
    (
        Command::Layer(Layering::Forward),
        MaterialSymbol::ARROW_UPWARD,
        "paint.command.bring_forward",
    ),
    (
        Command::Layer(Layering::Backward),
        MaterialSymbol::ARROW_DOWNWARD,
        "paint.command.send_backward",
    ),
    (
        Command::Layer(Layering::MergeDown),
        MaterialSymbol::CALL_MERGE,
        "paint.command.merge_down",
    ),
    (
        Command::Begin(Kind::LayerProperties),
        MaterialSymbol::TUNE,
        "paint.action.layer_properties",
    ),
    (
        Command::Layer(Layering::Delete),
        MaterialSymbol::DELETE,
        "paint.command.delete_layer",
    ),
];

struct Row {
    id: LayerId,
    name: Label,
    kind: LayerKind,
    visible: bool,
    depth: usize,
    open: bool,
    active_layer: bool,
    background: bool,
    aim: Option<Placement>,
    thumbnail: Option<ImageSource>,
}

pub(crate) fn list(paint: &Paint, tab: &Tab) -> impl View {
    let mut rows = Vec::new();
    gather(tab, tab.layers(), 0, &mut rows);
    let ids: Rc<[LayerId]> = rows.iter().map(|row| row.id).collect();
    let shapes: Rc<[Shape]> = rows
        .iter()
        .map(|row| Shape {
            depth: row.depth,
            group: row.kind == LayerKind::Group,
            open: row.open,
            background: row.background,
        })
        .collect();
    let rows: Rc<[Row]> = Rc::from(rows);
    let listed = lazy_column(rows.len(), move |slot| {
        layer_row(&rows[slot.index], slot.index, &ids, &shapes).build()
    })
    .uniform(ROW)
    .padding(Space::TIGHT)
    .grow(1.0)
    .fill_width();
    column! {
        listed;
        rule();
        footer(paint);
        if let Some((layer, at)) = tab.layer_menu() {
            menu_at(paint, layer, at);
        }
    }
    .fill()
}

fn gather(tab: &Tab, layers: &[Layer], depth: usize, rows: &mut Vec<Row>) {
    let active_layer = tab.active_layer().map(|layer| layer.id);
    let aim = tab.aim();
    for layer in layers {
        let open = layer.kind == LayerKind::Group && tab.is_open_group(layer.id);
        let aimed = aim.filter(|aim| {
            matches!(aim, Placement::Above(id) | Placement::Below(id) | Placement::Into(id) if *id == layer.id)
        });
        rows.push(Row {
            id: layer.id,
            name: wording::layer_name(layer),
            kind: layer.kind,
            visible: layer.visible,
            depth,
            open,
            active_layer: active_layer == Some(layer.id),
            background: layer.background,
            aim: aimed,
            thumbnail: tab.thumbnail(layer.id).cloned(),
        });
        if open {
            gather(tab, &layer.children, depth + 1, rows);
        }
    }
}

fn layer_row(row: &Row, index: usize, ids: &Rc<[LayerId]>, shapes: &Rc<[Shape]>) -> impl View {
    let id = row.id;
    let visible = row.visible;
    let disclosure = match row.kind {
        LayerKind::Group => {
            let symbol = match row.open {
                true => MaterialSymbol::EXPAND_MORE,
                false => MaterialSymbol::CHEVRON_RIGHT,
            };
            button(icon(symbol))
                .ghost()
                .small()
                .icon_only()
                .focusless()
                .on_press(move |paint: &mut Paint| paint.toggle_group(id))
                .width(TOGGLE)
                .any()
        }
        _ => spacer().width(TOGGLE).any(),
    };
    let eye = match visible {
        true => MaterialSymbol::VISIBILITY,
        false => MaterialSymbol::VISIBILITY_OFF,
    };
    let name = text(row.name.clone()).no_wrap().shrink(1.0);
    let name = match visible {
        true => name,
        false => name.muted(),
    };
    let face = row! {
        preview(row);
        name;
        spacer().grow(1.0);
        if row.background {
            icon(MaterialSymbol::LOCK)
                .extent(Length::px(GLYPH * 0.8))
                .color(palette().text_muted);
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Space::TIGHT);
    let row_view = row! {
        spacer().width(INDENT * row.depth as f32);
        disclosure;
        button(icon(eye))
            .ghost()
            .small()
            .icon_only()
            .focusless()
            .on_press(move |paint: &mut Paint| paint.show_layer(id, !visible))
            .width(TOGGLE)
            .tooltip(match visible {
                true => tr!("paint.action.hide_layer"),
                false => tr!("paint.action.show_layer"),
            });
        container(grab(face, index, Rc::clone(ids), Rc::clone(shapes), ROW).keyed((ROW_KEY, id)))
            .grow(1.0);
    }
    .gap(Space::HAIR)
    .align_items(Align::Center)
    .fill_width();
    let palette = palette();
    let tint = match row.active_layer {
        true => palette.accent.scale_alpha(ACTIVE_TINT),
        false => Color::TRANSPARENT,
    };
    let mark = |lit: bool| {
        container(spacer())
            .background(match lit {
                true => palette.accent,
                false => Color::TRANSPARENT,
            })
            .height(MARK)
            .fill_width()
    };
    let into = matches!(row.aim, Some(Placement::Into(_)));
    let border = match into {
        true => palette.accent,
        false => Color::TRANSPARENT,
    };
    column! {
        mark(matches!(row.aim, Some(Placement::Above(_))));
        container(row_view)
            .background(tint)
            .border(1.0, border)
            .radius(Corner::Item)
            .grow(1.0)
            .fill_width();
        mark(matches!(row.aim, Some(Placement::Below(_))));
    }
    .height(ROW)
    .fill_width()
}

fn preview(row: &Row) -> Element {
    let symbol = match row.kind {
        LayerKind::Group => Some(MaterialSymbol::FOLDER),
        LayerKind::Adjustment => Some(MaterialSymbol::TUNE),
        _ => None,
    };
    match (symbol, &row.thumbnail) {
        (None, Some(source)) => thumbnail(source.clone(), THUMBNAIL).any(),
        (symbol, _) => container(
            icon(symbol.unwrap_or(MaterialSymbol::IMAGE))
                .extent(Length::px(GLYPH))
                .color(palette().text_muted),
        )
        .center()
        .size(THUMBNAIL, THUMBNAIL)
        .any(),
    }
}

fn footer(paint: &Paint) -> impl View {
    let buttons: Vec<Element> = FOOTER
        .into_iter()
        .map(|(command, symbol, label)| {
            button(icon(symbol))
                .ghost()
                .small()
                .icon_only()
                .focusless()
                .enabled(command.enabled(paint))
                .on_press(move |paint: &mut Paint| command.run(paint))
                .tooltip(command.tooltip(tr!(label)))
        })
        .collect();
    row_of(buttons)
        .gap(Space::HAIR)
        .justify(Justify::Center)
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .fill_width()
}

fn menu_at(paint: &Paint, layer: LayerId, at: Point) -> Element {
    let entry = |command: Command, label: Label| command.entry(label, paint);
    menu()
        .items([
            entry(
                Command::Layer(Layering::New),
                tr!("paint.command.new_layer"),
            ),
            entry(
                Command::Layer(Layering::Duplicate),
                tr!("paint.command.duplicate_layer"),
            ),
            entry(
                Command::Layer(Layering::Delete),
                tr!("paint.command.delete_layer"),
            ),
            menu_separator(),
            entry(
                Command::Begin(Kind::LayerProperties),
                tr!("paint.command.layer_properties"),
            ),
            entry(
                Command::Layer(Layering::FromBackground),
                tr!("paint.command.layer_from_background"),
            ),
            menu_separator(),
            entry(
                Command::Layer(Layering::MergeDown),
                tr!("paint.command.merge_down"),
            ),
        ])
        .width(MENU)
        .on_activate(|paint: &mut Paint, _: usize| paint.close_layer_menu())
        .on_dismiss(Paint::close_layer_menu)
        .build()
        .keyed(("paint-layer-menu", layer))
        .anchored(Anchor::new((ROW_KEY, layer), AnchorEdge::BelowStart).offset(at.x, at.y))
}
