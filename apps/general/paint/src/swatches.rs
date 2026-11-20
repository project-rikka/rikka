use rikka_app::prelude::*;

use crate::colors::{Colors, Slot};
use crate::command::Command;
use crate::paint::Paint;

const WHOLE: f32 = 44.0;
const WELL: f32 = 28.0;
const BEHIND: f32 = WHOLE - WELL;
const SMALL: f32 = 14.0;
const CHIP: f32 = 8.0;
const CHIP_SHIFT: f32 = 4.0;

pub(crate) fn swatches(colors: Colors) -> impl View {
    column_of(vec![
        well(colors, Slot::Secondary)
            .position(BEHIND, BEHIND)
            .tooltip(beside(tr!("paint.color.secondary"))),
        well(colors, Slot::Primary)
            .position(0.0, 0.0)
            .tooltip(beside(tr!("paint.color.primary"))),
        container(
            icon(MaterialSymbol::SWAP_HORIZ)
                .extent(Length::px(SMALL))
                .color(palette().text_muted),
        )
        .center()
        .on_press(Paint::swap_colors)
        .focusless()
        .size(SMALL, SMALL)
        .position(WELL + 1.0, 0.0)
        .tooltip(
            Command::SwapColors
                .tooltip(tr!("paint.command.swap_colors"))
                .side(TooltipSide::After),
        ),
        resetting().position(0.0, WELL + 1.0).tooltip(
            Command::ResetColors
                .tooltip(tr!("paint.command.reset_colors"))
                .side(TooltipSide::After),
        ),
    ])
    .size(WHOLE, WHOLE)
}

fn beside(label: Label) -> Tooltip {
    Tooltip::new(label).side(TooltipSide::After)
}

fn well(colors: Colors, slot: Slot) -> impl Styled + View {
    color_well(colors.get(slot))
        .alpha(true)
        .on_change(move |paint: &mut Paint, color: Color| paint.set_color(slot, color))
        .on_commit(move |paint: &mut Paint, color: Color| paint.commit_color(slot, color))
        .width(WELL)
        .height(WELL)
}

fn resetting() -> impl Styled + View {
    let chip = |color: Color| {
        container(spacer())
            .background(color)
            .border(1.0, palette().text_muted)
            .size(CHIP, CHIP)
    };
    container(
        column_of(vec![
            chip(Color::WHITE).position(CHIP_SHIFT, CHIP_SHIFT).build(),
            chip(Color::BLACK).position(0.0, 0.0).build(),
        ])
        .size(SMALL, SMALL),
    )
    .on_press(Paint::reset_colors)
    .focusless()
    .size(SMALL, SMALL)
}
