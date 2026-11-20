use rikka_app::prelude::*;

use crate::internal::route::Presented;
use crate::menus::Entries;
use crate::model::Model;
use crate::shell::Shell;
use crate::sidebar::titled;
use browser_model::Arrangement;
use browser_model::{TabId, WindowId};

pub(crate) fn tab_entries(entries: &mut Entries, model: &Model, window: WindowId, tab: TabId) {
    if model.split_of(tab).is_some() {
        entries.leaf(
            menu_item(tr!("browser.split.separate"))
                .icon(MaterialSymbol::OPEN_IN_FULL)
                .on_press(move |shell: &mut Shell| shell.separate_pane(tab)),
        );
        return;
    }
    let active = model.window(window).and_then(|layout| layout.active());
    if active != Some(tab) && model.splittable_with(window, tab) {
        entries.leaf(
            menu_item(tr!("browser.split.add"))
                .icon(MaterialSymbol::SPLITSCREEN_ADD)
                .on_press(move |shell: &mut Shell| shell.add_to_split(tab)),
        );
    }
}

pub(crate) fn split_menu(model: &Model, window: WindowId) -> Entries {
    let mut entries = Entries::new();
    let Some(active) = model.window(window).and_then(|layout| layout.active()) else {
        return entries;
    };
    if model.splittable_with(window, active) {
        entries.leaf(
            menu_item(tr!("browser.split.new-tab"))
                .icon(MaterialSymbol::ADD)
                .on_press(|shell: &mut Shell| shell.split_with_new_tab()),
        );
    }
    let candidates: Vec<MenuItem> = model
        .split_candidates(window)
        .into_iter()
        .filter_map(|tab| {
            let tab_entry = model.tab(tab)?;
            let title = match &*tab_entry.internal {
                Some(internal) => internal.title().to_string(),
                None if tab_entry.is_blank() => tr!("browser.tab.new").to_string(),
                None => titled(&tab_entry.meta.title, &tab_entry.meta.url),
            };
            Some(menu_item(title).on_press(move |shell: &mut Shell| shell.add_to_split(tab)))
        })
        .collect();
    if !candidates.is_empty() {
        entries.branch(
            menu_item(tr!("browser.split.add"))
                .icon(MaterialSymbol::SPLITSCREEN_ADD)
                .items(candidates),
        );
    }
    let Some(group) = model.shown_split(window) else {
        return entries;
    };
    let id = group.id;
    entries.separator();
    for (arrangement, label, symbol) in [
        (
            Arrangement::SideBySide,
            tr!("browser.split.side-by-side"),
            MaterialSymbol::VIEW_COLUMN,
        ),
        (
            Arrangement::Stacked,
            tr!("browser.split.stacked"),
            MaterialSymbol::VIEW_AGENDA,
        ),
    ] {
        entries.leaf(
            menu_item(label)
                .icon(symbol)
                .selected(group.arrangement == arrangement)
                .on_press(move |shell: &mut Shell| shell.arrange_split(id, arrangement)),
        );
    }
    entries.separator();
    entries.leaf(
        menu_item(tr!("browser.split.separate-all"))
            .icon(MaterialSymbol::OPEN_IN_FULL)
            .on_press(move |shell: &mut Shell| shell.dissolve_split(id)),
    );
    entries
}
