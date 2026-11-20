use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::command::Command;
use crate::paint::Paint;
use crate::swatches;
use crate::tool::Tool;

const RAIL: f32 = 52.0;
const TILE: f32 = 40.0;
const GLYPH: f32 = 22.0;
const DIVIDER: f32 = 24.0;

pub(crate) fn rail(paint: &Paint) -> impl View {
    let mut tiles = Vec::new();
    let mut family = None;
    for tool in Tool::ALL {
        if family.is_some_and(|family| family != tool.family()) {
            tiles.push(
                rule()
                    .width(DIVIDER)
                    .margin(Inset::xy(Space::NONE, Space::TIGHT))
                    .build(),
            );
        }
        family = Some(tool.family());
        tiles.push(tool_button(tool, paint.tool() == tool).build());
    }
    container(
        column! {
            scrollable(
                column_of(tiles)
                    .gap(Space::TIGHT)
                    .padding(Space::SNUG)
                    .align_items(Align::Center)
                    .fill_width(),
            )
            .grow(1.0)
            .fill_width();
            swatches::swatches(paint.colors());
        }
        .align_items(Align::Center)
        .padding(Inset::xy(Space::NONE, Space::SNUG))
        .fill(),
    )
    .background(palette().surface)
    .width(RAIL)
    .fill_height()
}

pub(crate) fn menu(paint: &Paint) -> Vec<MenuItem> {
    let entry = |command: Command, label: Label| command.entry(label, paint);
    let mut items = Vec::new();
    let mut family = None;
    for tool in Tool::ALL {
        if family.is_some_and(|family| family != tool.family()) {
            items.push(menu_separator());
        }
        family = Some(tool.family());
        items.push(entry(Command::Pick(tool), tool.name()).selected(paint.tool() == tool));
    }
    items.extend([
        menu_separator(),
        entry(Command::SwapColors, tr!("paint.command.swap_colors")),
        entry(Command::ResetColors, tr!("paint.command.reset_colors")),
        menu_separator(),
        entry(Command::Thinner, tr!("paint.command.thinner")),
        entry(Command::Thicker, tr!("paint.command.thicker")),
    ]);
    items
}

fn tool_button(tool: Tool, selected: bool) -> impl View {
    tool_tile(tool.symbol(), GLYPH, selected)
        .on_press(move |paint: &mut Paint| paint.pick(tool))
        .size(TILE, TILE)
        .tooltip(
            Command::Pick(tool)
                .tooltip(tool.name())
                .side(TooltipSide::After),
        )
}
