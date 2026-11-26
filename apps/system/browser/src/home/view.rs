use browser_preferences::HomePart;
use rikka_app::prelude::*;

use crate::home::{HomePage, clock, closed, favorites, search, sites};
use crate::shell::Shell;
use browser_model::TabId;

const WIDEST: f32 = 760.0;
const ABOVE: f32 = 72.0;

pub(crate) fn view(tab: TabId) -> Element {
    component_with(|shell: &Shell| shell, tab, page).any()
}

fn page(shell: &Shell, tab: &TabId) -> Element {
    let tab = *tab;
    let preferences = shell.browser().preferences();
    let parts: Vec<Element> = HomePart::ALL
        .into_iter()
        .filter(|part| preferences.home_shows(*part))
        .filter_map(|part| part_view(shell, part, tab))
        .collect();
    let content = column_of(parts)
        .gap(Space::WIDE)
        .align_items(Align::Center)
        .padding(Inset::edges(ABOVE, Space::LOOSE, Space::WIDE, Space::LOOSE))
        .max_width(WIDEST)
        .fill_width();
    container(scrollable(
        container(content).align_x(Align::Center).fill_width(),
    ))
    .background(palette().background)
    .fill()
    .any()
}

fn part_view(shell: &Shell, part: HomePart, tab: TabId) -> Option<Element> {
    match part {
        HomePart::Clock => Some(component(|page: &HomePage| page, clock::view).any()),
        HomePart::Search => Some(component_with(|shell: &Shell| shell, tab, search::view).any()),
        HomePart::TopSites => sites::view(shell, tab),
        HomePart::Favorites => favorites::view(shell, tab),
        HomePart::Closed => closed::view(shell, tab),
    }
}
