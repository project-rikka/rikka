use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::shell::Shell;
use crate::sidebar::look::{EDGE, PANEL};
use crate::sidebar::{favorites, pinned, space, today};

pub(crate) fn panel(shell: &Shell) -> Element {
    let (at, space) = space::slot(shell);
    column! {
        component(|shell: &Shell| shell, favorites::view);
        page_stack(at, component(|shell: &Shell| shell, body))
            .named(space.map(|space| space.get().unsigned_abs()))
            .grow(1.0)
            .fill_width();
        component(|shell: &Shell| shell, space::switcher);
    }
    .gap(Space::TIGHT)
    .padding(Inset::all(EDGE))
    .width(PANEL)
    .fill_height()
    .any()
}

fn body(shell: &Shell) -> Element {
    let pinned = (!shell.private()).then(|| component(|shell: &Shell| shell, pinned::view));
    column! {
        component(|shell: &Shell| shell, space::title);
        scrollable(
            column! {
                pinned;
                component(|shell: &Shell| shell, today::view);
            }
            .gap(Space::TIGHT)
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .gap(Space::TIGHT)
    .fill()
    .any()
}
