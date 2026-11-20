use rikka_types::{Key, Modifiers};
use rikka_ui::{Element, Styled, View};
use rikka_widgets::menu;
use shell_kit::popover;

use crate::menu::WIDTH;
use crate::menu::flyout::Flyout;
use crate::menu::rows::rows;

#[must_use]
pub(crate) fn view(flyout: &Flyout) -> Element {
    let panel = menu()
        .items(rows(flyout.on_widget()).into_iter().map(|row| row.item))
        .width(WIDTH)
        .on_activate(|flyout: &mut Flyout, row: usize| flyout.chose(row))
        .still();

    popover(panel, Flyout::dismiss)
        .beside(flyout.aim())
        .width(WIDTH)
        .on_reopen(Flyout::reopen)
        .on_key(|flyout: &mut Flyout, key: Key, _: Modifiers| {
            let dismissed = key == Key::Escape;
            if dismissed {
                flyout.dismiss();
            }
            dismissed
        })
        .build()
}
