use rikka_types::{Key, Modifiers};
use rikka_ui::{Styled, View, is_exiting};
use rikka_widgets::column;
use shell_kit::scrim;

use crate::dropdown_panel::dropdown_panel;
use crate::indicators::panel::WIDTH;
use crate::indicators::panel::state::Drawer;

pub(crate) fn view(drawer: &Drawer) -> impl View {
    let shelf = drawer.shelf();
    let open = shelf.open_indicator().filter(|_| !is_exiting());
    scrim(
        column! {
            if let Some(indicator) = open {
                dropdown_panel(shelf.from(), WIDTH, vec![indicator.panel(shelf)], on_key)
                    .build()
                    .keyed(indicator.kind());
            }
        }
        .fill(),
        Drawer::dismiss,
    )
}

fn on_key(drawer: &mut Drawer, key: Key, _modifiers: Modifiers) -> bool {
    if key != Key::Escape {
        return false;
    }
    drawer.dismiss();
    true
}
