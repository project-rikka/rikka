use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::row;

use crate::content;
use crate::menus::{self, MARK, Menus};
use crate::shell::Shell;
use crate::site_info::{self, security_glyph};
use crate::widgets::{context_area, favicon};

const GLYPH: f32 = 15.0;
const TOOL: f32 = 18.0;

pub(crate) fn view(_: &Shell) -> Element {
    let layers = column_of([
        row! {
            spacer().width(0.0).fill_height().build().keyed(MARK);
            content::column()
            .grow(1.0)
            .fill_height();
        }
        .fill()
        .any(),
        component(|shell: &Shell| shell, menus::view),
        component(|shell: &Shell| shell, crate::overlays::view),
    ])
    .fill();
    let open = Handler::new(|menus: &mut Menus, at: Point| menus.open_at(at));
    context_area(layers, open)
}

pub(crate) fn caption_bar(_: &Shell) -> Option<CaptionBar> {
    Some(
        caption()
            .leading(component(|shell: &Shell| shell, site))
            .fill(spacer().fill_width())
            .trailing(component(|shell: &Shell| shell, tools))
            .into(),
    )
}

fn site(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(tab_entry) = model.active(shell.id()) else {
        return Element::empty();
    };
    let (symbol, tint) = security_glyph(&tab_entry.meta.security);
    let shown = crate::address::display_address(&tab_entry.meta.url);
    let web = browser_pages::web_host(&tab_entry.meta.url).is_some();
    let capture = *tab_entry.meta.capture;
    row! {
        favicon(tab_entry.meta.favicon.as_ref(), MaterialSymbol::PUBLIC, GLYPH);
        site_info::badge(symbol, tint, web);
        site_info::capturing(capture);
        text(shown.host).label().no_wrap();
        text(shown.rest).label().muted().no_wrap().shrink(1.0).min_width(0.0);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .any()
}

fn tools(shell: &Shell) -> Element {
    let loading = shell
        .model()
        .active(shell.id())
        .is_some_and(|tab_entry| *tab_entry.meta.loading);
    let reload = match loading {
        true => button(icon(MaterialSymbol::CLOSE).extent(Length::px(TOOL)))
            .on_press(|shell: &mut Shell| shell.stop()),
        false => button(icon(MaterialSymbol::REFRESH).extent(Length::px(TOOL)))
            .on_press(|shell: &mut Shell| shell.reload(false)),
    };
    row! {
        reload.ghost().icon_only().small().focusless();
        component(|shell: &Shell| shell, crate::downloads::button);
        button(
            row! {
                icon(MaterialSymbol::OPEN_IN_BROWSER).extent(Length::px(16.0));
                text(tr!("browser.little.open-in-browser")).label();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .ghost()
        .small()
        .focusless()
        .on_press(|shell: &mut Shell| shell.move_into_browser());
    }
    .gap(Space::HAIR)
    .align_items(Align::Center)
    .any()
}
