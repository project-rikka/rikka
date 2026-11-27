use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::shell::Shell;
use crate::widgets::grip;

const BACK: &str = "rikka-browser-back";
const FORWARD: &str = "rikka-browser-forward";
const SIDEBAR: &str = "rikka-browser-sidebar-toggle";
const SIDE: f32 = 28.0;
const GLYPH: f32 = 18.0;

pub(crate) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let tab_entry = model.active(shell.id());
    let back = tab_entry.is_some_and(|tab_entry| {
        *tab_entry.meta.back || (tab_entry.internal.is_some() && !tab_entry.meta.url.is_empty())
    });
    let forward = tab_entry.is_some_and(|tab_entry| *tab_entry.meta.forward);
    let loading =
        tab_entry.is_some_and(|tab_entry| *tab_entry.meta.loading && tab_entry.internal.is_none());
    let (sidebar, toggles) = match shell.browser().preferences().sidebar_expanded() {
        true => (
            MaterialSymbol::LEFT_PANEL_CLOSE,
            tr!("browser.sidebar.collapse"),
        ),
        false => (
            MaterialSymbol::LEFT_PANEL_OPEN,
            tr!("browser.sidebar.expand"),
        ),
    };
    row! {
        tool(sidebar, true)
            .tip(toggles.to_string(), TooltipSide::Below)
            .on_press(|shell: &mut Shell| shell.toggle_sidebar())
            .build()
            .keyed(SIDEBAR);
        tool(MaterialSymbol::ARROW_BACK, back)
            .on_press(|shell: &mut Shell| shell.back())
            .build()
            .keyed(BACK);
        tool(MaterialSymbol::ARROW_FORWARD, forward)
            .on_press(|shell: &mut Shell| shell.forward())
            .build()
            .keyed(FORWARD);
        match loading {
            true => tool(MaterialSymbol::CLOSE, true).on_press(|shell: &mut Shell| shell.stop()).any(),
            false => tool(MaterialSymbol::REFRESH, tab_entry.is_some())
                .on_press(|shell: &mut Shell| shell.reload(false))
                .any(),
        };
    }
    .gap(Space::HAIR)
    .align_items(Align::Center)
    .any()
}

fn tool(symbol: MaterialSymbol, enabled: bool) -> crate::widgets::GripView {
    let tint = match enabled {
        true => palette().text,
        false => palette().text_muted.with_alpha(0.5),
    };
    grip(
        container(icon(symbol).extent(Length::px(GLYPH)).color(tint))
            .center()
            .fill(),
    )
    .size(Length::px(SIDE), Length::px(SIDE))
}
