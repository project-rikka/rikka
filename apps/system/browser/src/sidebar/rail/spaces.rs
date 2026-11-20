use rikka_app::prelude::*;

use crate::menus::{Menus, Subject, space_name};
use crate::shell::Shell;
use crate::sidebar::look::{space_mark, space_tint};
use crate::sidebar::rail::cells::{cell, glyph};
use crate::sidebar::rail::place::{CELL, GLYPH, RailCell, key_of};

pub(super) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(current) = model.window(shell.id()).and_then(|layout| layout.space()) else {
        return container(glyph(MaterialSymbol::VISIBILITY_OFF, palette().text))
            .center()
            .size(Length::px(CELL), Length::px(CELL))
            .any();
    };
    let marks = model.spaces.list().iter().map(|space| {
        let id = space.id;
        let chosen = id == current;
        cell(
            space_mark(&space.icon, GLYPH, space_tint(chosen)),
            space_name(&space.name),
        )
        .highlighted(chosen)
        .on_press(move |shell: &mut Shell| shell.switch_space(id))
        .aim(move |menus: &mut Menus| menus.aim(Subject::Space(id)))
        .build()
        .keyed(key_of(RailCell::Space(id)))
    });
    let adding = cell(
        glyph(MaterialSymbol::ADD, palette().text_muted),
        tr!("browser.sidebar.new-space").to_string(),
    )
    .on_press(|shell: &mut Shell| shell.new_space())
    .build()
    .keyed(key_of(RailCell::NewSpace));
    column_of(marks.chain(std::iter::once(adding)))
        .gap(Space::HAIR)
        .align_items(Align::Center)
        .any()
}
