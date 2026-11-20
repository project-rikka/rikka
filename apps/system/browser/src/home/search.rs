use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::shell::Shell;
use browser_model::TabId;

const HEIGHT: f32 = 48.0;
const GLYPH: f32 = 20.0;
const RING: f32 = 2.0;
const WIDEST: f32 = 560.0;

pub(crate) fn view(shell: &Shell, tab: &TabId) -> Element {
    let tab = *tab;
    let engines = shell.browser().search_engines();
    let engine = engines.default_engine().name().to_owned();
    let focused = shell.address().editing() && shell.active_tab() == Some(tab);
    let ring = match focused {
        true => palette().accent,
        false => Color::TRANSPARENT,
    };
    container(
        row! {
            icon(MaterialSymbol::SEARCH)
                .extent(Length::px(GLYPH))
                .color(palette().text_muted);
            text(tr!("browser.home.search", engine = &engine))
                .body()
                .muted()
                .no_wrap()
                .min_width(0.0)
                .grow(1.0);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill(),
    )
    .padding(Inset::xy(Space::ROOMY, Space::NONE))
    .radius(HEIGHT / 2.0)
    .background(palette().text.with_alpha(0.06))
    .border(RING, ring)
    .on_press(move |shell: &mut Shell| shell.search_from_home(tab))
    .focusless()
    .height(HEIGHT)
    .max_width(WIDEST)
    .fill_width()
    .any()
}
