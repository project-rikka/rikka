use rikka_app::prelude::*;

use crate::command::Command;
use crate::cut::Cut;

pub(crate) fn glyph_button(symbol: MaterialSymbol, glyph: f32) -> Button {
    button(icon(symbol).extent(Length::px(glyph)))
        .icon_only()
        .small()
        .focusless()
}

pub(crate) fn tool(symbol: MaterialSymbol, glyph: f32, command: Command, cut: &Cut) -> impl View {
    glyph_button(symbol, glyph)
        .ghost()
        .enabled(command.enabled(cut))
        .on_press(move |cut: &mut Cut| command.run(cut))
        .tooltip(command.tooltip())
}

pub(crate) fn toggle<M>(
    symbol: MaterialSymbol,
    glyph: f32,
    on: bool,
    press: impl IntoHandler<(), (), M>,
) -> impl View {
    let button = glyph_button(symbol, glyph).on_press(press);
    match on {
        true => button.secondary(),
        false => button.ghost(),
    }
}

pub(crate) fn corner(width: f32, height: f32) -> impl View {
    container(spacer())
        .background(palette().surface)
        .size(width, height)
}
