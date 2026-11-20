use rikka_types::{Key, Modifiers};
use rikka_ui::{Element, Styled, View};
use rikka_widgets::{menu, spacer};
use shell_kit::popover;

use crate::tasks::menu::flyout::Flyout;
use crate::tasks::menu::rows::rows;

const WIDTH: f32 = 300.0;

#[must_use]
pub(crate) fn view(flyout: &Flyout) -> Element {
    let Some(request) = flyout.request() else {
        return spacer().build();
    };
    let panel = menu().items(rows(request)).width(WIDTH).still();

    popover(panel, Flyout::dismiss)
        .at(request.at)
        .width(WIDTH)
        .on_key(|flyout: &mut Flyout, key: Key, _: Modifiers| {
            let dismissed = key == Key::Escape;
            if dismissed {
                flyout.dismiss();
            }
            dismissed
        })
        .build()
}
