mod face;
mod favorites;
mod look;
mod panel;
mod pinned;
mod rail;
mod rename;
mod row;
mod space;
mod state;
mod today;

pub(crate) use face::{Face, titled};
pub(crate) use rail::cells::cell as rail_grip;
pub(crate) use rail::naming;
pub(crate) use rename::field as rename_field;
pub(crate) use state::{Renaming, Sidebar};

use rikka_app::prelude::*;

use crate::corners::Corners;
use crate::gestures::space_swipe;
use crate::shell::Shell;
use crate::sidebar::look::{PANEL, RAIL};

const RAIL_FACE: &str = "rikka-browser-sidebar-rail";
const PANEL_FACE: &str = "rikka-browser-sidebar-panel";

pub(crate) fn side(shell: &Shell) -> Element {
    let (face, width) = match expanded(shell) {
        true => (
            component(|shell: &Shell| shell, panel::panel)
                .transition(fade())
                .keyed(PANEL_FACE),
            PANEL,
        ),
        false => (
            component(|shell: &Shell| shell, rail::rail)
                .transition(fade())
                .keyed(RAIL_FACE),
            RAIL,
        ),
    };
    container(space_swipe(face, swipe_spaces))
        .background(Corners::for_shell(shell).sidebar())
        .clip_content()
        .width(width)
        .fill_height()
        .any()
}

pub(crate) fn expanded(shell: &Shell) -> bool {
    shell.browser().preferences().sidebar_expanded()
}

fn swipe_spaces(shell: &mut Shell, by: isize) {
    shell.step_space(by);
}
