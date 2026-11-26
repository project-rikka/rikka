use browser_preferences::HomePart;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::shell::Shell;

pub(crate) fn view(shell: &Shell) -> Element {
    let preferences = shell.browser().preferences();
    let switches = HomePart::ALL.map(|part| {
        field(
            part_name(part),
            switch(preferences.home_shows(part)).on_toggle(move |shell: &mut Shell, on: bool| {
                shell.browser().preferences().set_home_shows(part, on);
            }),
        )
    });
    page([section(
        tr!("browser.home.title"),
        column! {
            column_of(switches).gap(Space::TIGHT).fill_width();
            row! {
                button(text(tr!("browser.home.show-hidden")))
                    .secondary()
                    .small()
                    .enabled(shell.persistent_profile().is_some())
                    .on_press(|shell: &mut Shell| shell.show_hidden_top_sites());
            };
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )])
    .any()
}

fn part_name(part: HomePart) -> Label {
    match part {
        HomePart::Clock => tr!("browser.home.clock"),
        HomePart::Search => tr!("browser.home.search-box"),
        HomePart::TopSites => tr!("browser.home.top-sites"),
        HomePart::Favorites => tr!("browser.home.favorites"),
        HomePart::Closed => tr!("browser.home.closed"),
    }
}
