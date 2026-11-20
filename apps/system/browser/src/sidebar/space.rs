use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::menus::{Menus, Subject, space_name};
use crate::shell::Shell;
use crate::sidebar::look::{ROW, space_mark, space_tint};
use crate::sidebar::rename;
use crate::sidebar::state::Renaming;
use crate::widgets::glyph_button;
use browser_model::SpaceId;

pub(crate) const SPACE_MENU: &str = "rikka-browser-space-menu";
const MARK: f32 = 18.0;
const SWITCH: f32 = 28.0;

pub(crate) fn title(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(space) = model.space_of(shell.id()) else {
        return private_title();
    };
    let id = space.id;
    let renaming = shell.sidebar().renaming() == Some(Renaming::Space(id));
    let name = match renaming {
        true => rename::field(shell),
        false => text(space_name(&space.name))
            .label()
            .semibold()
            .no_wrap()
            .grow(1.0)
            .min_width(0.0)
            .any(),
    };
    row! {
        space_mark(&space.icon, MARK, palette().text);
        name;
        glyph_button(MaterialSymbol::MORE_HORIZ, 18.0)
            .on_press(move |menus: &mut Menus| menus.open_below(Subject::Space(id), SPACE_MENU))
            .build()
            .keyed(SPACE_MENU);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .height(ROW + 4.0)
    .fill_width()
    .any()
}

fn private_title() -> Element {
    row! {
        icon(MaterialSymbol::VISIBILITY_OFF).extent(Length::px(MARK)).color(palette().text);
        text(tr!("browser.private")).label().semibold().no_wrap();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .height(ROW + 4.0)
    .fill_width()
    .any()
}

pub(crate) fn switcher(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(current) = model.window(shell.id()).and_then(|layout| layout.space()) else {
        return Element::empty();
    };
    let spaces = model.spaces.list().iter().map(|space| {
        let id = space.id;
        let chosen = id == current;
        container(space_mark(&space.icon, 16.0, space_tint(chosen)))
            .center()
            .size(Length::px(SWITCH), Length::px(SWITCH))
            .radius(SWITCH / 2.0)
            .background(match chosen {
                true => palette().text.with_alpha(0.12),
                false => Color::TRANSPARENT,
            })
            .on_press(move |shell: &mut Shell| shell.switch_space(id))
            .focusless()
            .build()
            .keyed(id)
    });
    row! {
        row_of(spaces).gap(Space::TIGHT).align_items(Align::Center).grow(1.0).min_width(0.0);
        glyph_button(MaterialSymbol::ADD, 18.0)
            .on_press(|shell: &mut Shell| shell.new_space());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::SNUG))
    .fill_width()
    .any()
}

pub(crate) fn slot(shell: &Shell) -> (usize, Option<SpaceId>) {
    let model = shell.model();
    let space = model.window(shell.id()).and_then(|layout| layout.space());
    let at = space
        .and_then(|space| model.spaces.position(space))
        .unwrap_or(0);
    (at, space)
}
