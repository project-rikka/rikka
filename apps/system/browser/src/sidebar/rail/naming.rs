use rikka_app::prelude::*;

use crate::shell::Shell;
use crate::sidebar::rail::place::{RailCell, beside};
use crate::sidebar::rename;
use crate::sidebar::state::Renaming;
use crate::widgets::presence;
use browser_model::{PinnedItem, Row};

const KEY: &str = "rikka-browser-rail-naming";
const WIDTH: f32 = 220.0;

pub(crate) fn view(shell: &Shell) -> Element {
    presence(field(shell))
}

fn field(shell: &Shell) -> Option<Element> {
    if crate::sidebar::expanded(shell) || shell.overlays.hides_chrome() {
        return None;
    }
    let named = match shell.sidebar().renaming()? {
        Renaming::Space(space) => RailCell::Space(space),
        Renaming::Folder(folder) => RailCell::Row(Row::Pin(PinnedItem::Folder(folder))),
        Renaming::Profile(_) => return None,
    };
    Some(
        popover(container(rename::field(shell)).width(WIDTH))
            .on_dismiss(|shell: &mut Shell| shell.finish_renaming())
            .build()
            .keyed(KEY)
            .anchored(beside(named)),
    )
}
