pub(crate) mod cells;
mod naming;
mod place;
mod sections;
mod spaces;

pub(crate) use naming::view as naming;

use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::shell::Shell;
use crate::sidebar::look::{EDGE, RAIL, rule};
use crate::sidebar::space;

pub(crate) fn rail(shell: &Shell) -> Element {
    let (at, space) = space::slot(shell);
    column! {
        component(|shell: &Shell| shell, sections::favorites);
        page_stack(at, component(|shell: &Shell| shell, body))
            .named(space.map(|space| space.get().unsigned_abs()))
            .grow(1.0)
            .fill_width();
        rule();
        component(|shell: &Shell| shell, spaces::view);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::NONE, EDGE))
    .width(RAIL)
    .fill_height()
    .any()
}

fn body(shell: &Shell) -> Element {
    let pinned = (!shell.private()).then(|| component(|shell: &Shell| shell, sections::pinned));
    scrollable(
        column! {
            pinned;
            cells::new_tab();
            component(|shell: &Shell| shell, sections::today);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
        .fill_width(),
    )
    .fill()
    .any()
}
