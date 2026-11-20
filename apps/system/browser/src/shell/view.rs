use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::row;

use crate::menus::{self, MARK, Menus};
use crate::shell::Shell;
use crate::widgets::context_area;

pub(crate) fn view(shell: &Shell) -> Element {
    if shell.little() {
        return crate::little::view(shell);
    }
    let body = row! {
        component(|shell: &Shell| shell, crate::overlays::sidebar);
        component(|shell: &Shell| shell, crate::corners::page)
            .grow(1.0)
            .fill_height();
    }
    .fill();
    let layers = column_of([
        row! {
            spacer().width(0.0).fill_height().build().keyed(MARK);
            body;
        }
        .fill()
        .any(),
        component(|shell: &Shell| shell, crate::sidebar::naming),
        component(|shell: &Shell| shell, menus::view),
        component(|shell: &Shell| shell, crate::overlays::view),
        component(
            |bar: &crate::command_bar::CommandBar| bar,
            crate::command_bar::view,
        ),
    ])
    .fill();
    let open = Handler::new(|menus: &mut Menus, at: Point| menus.open_at(at));
    context_area(layers, open)
}
