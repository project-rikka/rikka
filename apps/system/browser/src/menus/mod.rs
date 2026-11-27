mod items;
mod page;
mod state;

pub(crate) use items::{Entries, profile_name, space_name};
pub(crate) use state::{MARK, Menus, Subject};

use rikka_app::prelude::*;

use crate::shell::Shell;
use crate::widgets::presence;

const WIDTH: f32 = 248.0;
const MENU: &str = "rikka-browser-menu";

pub(crate) fn view(shell: &Shell) -> Element {
    presence(panel(shell))
}

fn panel(shell: &Shell) -> Option<Element> {
    let shown = shell.menus().shown()?;
    let model = shell.model();
    let window = shell.id();
    let entries = match &shown.subject {
        Subject::Tab(tab) => items::tab(&model, window, *tab),
        Subject::Pin(pin) => items::pin(&model, window, *pin),
        Subject::Folder(folder) => items::folder(&model, *folder),
        Subject::Favorite(favorite) => items::favorite(&model, window, *favorite),
        Subject::TopSite(site) => crate::home::top_site_menu(shell, site),
        Subject::Space(space) => items::space(&model, *space),
        Subject::Main => items::main(shell.private()),
        Subject::ClearHistory => items::clear_history(),
        Subject::Split => crate::split::split_menu(&model, window),
        Subject::Page(tab) => page::entries(shell, *tab, shell.menus().page()?),
    };
    let (items, branches) = entries.finish();
    if items.is_empty() {
        return None;
    }
    Some(
        menu()
            .items(items)
            .width(WIDTH)
            .on_activate(move |menus: &mut Menus, index: usize| {
                if !branches.get(index).copied().unwrap_or(false) {
                    menus.close();
                }
            })
            .on_dismiss(|menus: &mut Menus| menus.close())
            .build()
            .keyed((MENU, shown.key))
            .anchored(shown.anchor),
    )
}
