use rikka_app::prelude::*;
use rikka_figure::ShapeKind;

use crate::command::Command;
use crate::cut::{Cut, Popup};
use crate::tool::Tool;
use crate::wording;

const GLYPH: f32 = 18.0;
const CHEVRON: f32 = 14.0;
const TILE: f32 = 30.0;
const NARROW: f32 = 16.0;
const MENU: f32 = 200.0;
const SHAPES: &str = "cut-range-shapes";

pub(crate) fn tools(cut: &Cut) -> impl View {
    let armed = cut.tool();
    let tiles = |tools: &[Tool]| {
        tool_group(
            tools
                .iter()
                .map(|tool| tile(*tool, armed == *tool, cut))
                .collect::<Vec<_>>(),
        )
    };
    let (picking, rest) = Tool::ALL.split_at(2);
    let (text, drawing) = rest.split_at(1);
    let bar = toolbar()
        .divided()
        .gap(Space::TIGHT)
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .group(tiles(picking).priority(ToolPriority::High))
        .group(tiles(text))
        .group(tiles(drawing))
        .group(range_tools(cut))
        .background(palette().surface);
    let popup = (cut.popup() == Some(&Popup::RangeShape)).then(|| shapes(cut.range_shape()));
    column_of(std::iter::once(bar).chain(popup)).fill_width()
}

fn tile(tool: Tool, armed: bool, cut: &Cut) -> ToolItem {
    let face = tool_tile(tool.symbol(), GLYPH, armed)
        .on_press(move |cut: &mut Cut| cut.set_tool(tool))
        .size(TILE, TILE)
        .tooltip(Command::Arm(tool).tooltip());
    tool_item(face).overflow([Command::Arm(tool)
        .entry(cut)
        .icon(tool.symbol())
        .selected(armed)])
}

fn range_tools(cut: &Cut) -> ToolGroup {
    let range = Tool::Range(cut.range_shape());
    let chevron = container(
        icon(MaterialSymbol::EXPAND_MORE)
            .extent(Length::px(CHEVRON))
            .color(palette().text_muted),
    )
    .radius(Corner::Item)
    .center()
    .on_press(|cut: &mut Cut| cut.open_popup(Popup::RangeShape))
    .focusless()
    .size(NARROW, TILE)
    .tooltip(tr!("cut.tool.range_shapes"))
    .keyed(SHAPES);
    tool_group([
        tile(range, matches!(cut.tool(), Tool::Range(_)), cut),
        tool_item(chevron).overflow([menu_item(tr!("cut.tool.range_shapes"))
            .icon(range.symbol())
            .items(shape_items(cut.range_shape()))]),
    ])
}

fn shape_items(selected: ShapeKind) -> Vec<MenuItem> {
    ShapeKind::ALL
        .into_iter()
        .map(|kind| {
            menu_item(wording::shape(kind))
                .icon(Tool::Shape(kind).symbol())
                .selected(kind == selected)
                .on_press(move |cut: &mut Cut| cut.set_tool(Tool::Range(kind)))
        })
        .collect()
}

fn shapes(selected: ShapeKind) -> Element {
    menu()
        .items(shape_items(selected))
        .width(MENU)
        .on_activate(|cut: &mut Cut, _: usize| cut.close_popup())
        .on_dismiss(Cut::close_popup)
        .build()
        .keyed("cut-range-shape-menu")
        .anchored(Anchor::new(SHAPES, AnchorEdge::BelowStart))
}
