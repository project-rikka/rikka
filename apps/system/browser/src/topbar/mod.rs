mod address;
mod navigation;

pub(crate) use address::{Address, FIELD};

use rikka_app::prelude::*;

use crate::menus::{Menus, Subject};
use crate::shell::Shell;
use crate::widgets::glyph_button;

const MAIN_MENU: &str = "rikka-browser-main-menu";
const ADDRESS: f32 = 760.0;

pub(crate) fn caption_bar(_: &Shell) -> Option<CaptionBar> {
    Some(
        caption()
            .leading(component(|shell: &Shell| shell, navigation::view))
            .fill(
                container(
                    component(|shell: &Shell| shell, address::view)
                        .max_width(ADDRESS)
                        .fill_width(),
                )
                .center()
                .fill_width(),
            )
            .trailing(
                row_of([
                    component(|shell: &Shell| shell, crate::split::button).any(),
                    component(|shell: &Shell| shell, crate::downloads::button).any(),
                    glyph_button(MaterialSymbol::MORE_VERT, 18.0)
                        .on_press(|menus: &mut Menus| menus.open_below(Subject::Main, MAIN_MENU))
                        .build()
                        .keyed(MAIN_MENU),
                ])
                .gap(Space::HAIR)
                .align_items(Align::Center),
            )
            .into(),
    )
}
