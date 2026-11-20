use rikka_app::prelude::*;

use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::widgets::grip;

const KEY: &str = "rikka-browser-split";
const SIDE: f32 = 28.0;
const GLYPH: f32 = 18.0;

pub(crate) fn button(shell: &Shell) -> Element {
    let model = shell.model();
    let shown = model.shown_split(shell.id()).is_some();
    let usable = shown
        || model
            .window(shell.id())
            .and_then(|layout| layout.active())
            .is_some_and(|tab| model.splittable_with(shell.id(), tab));
    if !usable {
        return Element::empty();
    }
    let tint = match shown {
        true => palette().accent,
        false => palette().text,
    };
    let gripped = grip(
        container(
            icon(MaterialSymbol::SPLITSCREEN)
                .extent(Length::px(GLYPH))
                .color(tint),
        )
        .center()
        .fill(),
    )
    .highlighted(shown)
    .aim(|menus: &mut Menus| menus.open_below(Subject::Split, KEY))
    .size(Length::px(SIDE), Length::px(SIDE));
    match shown {
        true => gripped.on_press(|menus: &mut Menus| menus.open_below(Subject::Split, KEY)),
        false => gripped.on_press(|shell: &mut Shell| shell.split_with_new_tab()),
    }
    .build()
    .keyed(KEY)
}
