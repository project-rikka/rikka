use browser_pages::site_details;
use rikka_app::prelude::*;
use rikka_web::DataKinds;
use rikka_widgets::{column, row};
use url::Url;

use crate::shell::Shell;
use browser_model::{Security, TabId};

pub(crate) const BADGE: &str = "rikka-browser-site-badge";
pub(crate) const PEEK_BADGE: &str = "rikka-browser-peek-badge";
const GLYPH: f32 = 15.0;
const WIDTH: f32 = 320.0;
const SITE_DATA: DataKinds = DataKinds {
    history: false,
    cookies: true,
    cache: true,
    downloads: false,
    form_data: false,
    passwords: false,
    site_storage: true,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Clearing {
    #[default]
    Idle,
    Busy,
    Done,
    Failed,
}

#[derive(State)]
pub(crate) struct SiteInfo {
    clearing: Tracked<Clearing>,
}

impl SiteInfo {
    pub(crate) fn new() -> SiteInfo {
        SiteInfo {
            clearing: Tracked::new(Clearing::Idle),
        }
    }
}

impl std::fmt::Debug for SiteInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SiteInfo")
            .field("clearing", &*self.clearing)
            .finish()
    }
}

fn origin_of(url: &str) -> Option<String> {
    let origin = Url::parse(url).ok()?.origin();
    origin.is_tuple().then(|| origin.ascii_serialization())
}

impl Shell {
    pub(crate) fn reset_site_info(&mut self) {
        self.overlays.site.clearing.set(Clearing::Idle);
    }

    fn clear_site_data(&mut self, tab: TabId) {
        let Some((key, origin)) = self
            .model()
            .tab(tab)
            .and_then(|tab_entry| Some((tab_entry.profile, origin_of(&tab_entry.meta.url)?)))
        else {
            return;
        };
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        self.overlays.site.clearing.set(Clearing::Busy);
        spawn(
            runtime.storage().clear_sites(&[origin], SITE_DATA),
            move |shell: &mut Shell, cleared: rikka_web::Result<()>| {
                let clearing = match cleared {
                    Ok(()) => Clearing::Done,
                    Err(error) => {
                        tracing::warn!(%error, "failed to clear site data");
                        Clearing::Failed
                    }
                };
                shell.overlays.site.clearing.set(clearing);
            },
        );
    }

    fn open_site_settings(&mut self, origin: &str) {
        self.overlays.close_popup();
        self.open_page(site_details(origin));
    }
}

pub(crate) fn security_glyph(security: Security) -> (MaterialSymbol, Color) {
    match security {
        Security::Secure => (MaterialSymbol::LOCK, palette().text_muted),
        Security::Insecure => (MaterialSymbol::LOCK_OPEN, palette().warning),
        Security::Unknown => (MaterialSymbol::PUBLIC, palette().text_muted),
    }
}

pub(crate) fn badge(symbol: MaterialSymbol, tint: Color, web: bool) -> Element {
    let glyph = icon(symbol).extent(Length::px(GLYPH)).color(tint);
    if !web {
        return container(glyph).build().keyed(BADGE);
    }
    button(glyph)
        .ghost()
        .icon_only()
        .small()
        .focusless()
        .on_press(|shell: &mut Shell| shell.open_site_info())
        .build()
        .keyed(BADGE)
}

pub(crate) fn peek_badge(tab: TabId, security: Security, size: f32) -> Element {
    let (symbol, tint) = security_glyph(security);
    button(icon(symbol).extent(Length::px(size)).color(tint))
        .ghost()
        .icon_only()
        .small()
        .focusless()
        .on_press(move |shell: &mut Shell| shell.open_site_info_of(tab))
        .build()
        .keyed(PEEK_BADGE)
}

pub(crate) fn view(shell: &Shell, tab: TabId) -> Element {
    let model = shell.model();
    let Some(tab_entry) = model.tab(tab) else {
        return Element::empty();
    };
    let url = (*tab_entry.meta.url).clone();
    let security = *tab_entry.meta.security;
    drop(model);
    let Some(origin) = origin_of(&url) else {
        return Element::empty();
    };
    let host = crate::address::display_address(&url).host;
    let (symbol, tint) = security_glyph(security);
    let connection = match security {
        Security::Secure => tr!("browser.site.secure"),
        Security::Insecure => tr!("browser.site.insecure"),
        Security::Unknown => tr!("browser.site.unknown"),
    };
    let clearing = *shell.overlays.site.clearing;
    let summary = match clearing {
        Clearing::Idle => tr!("browser.site.data-kept"),
        Clearing::Busy => tr!("browser.site.clearing"),
        Clearing::Done => tr!("browser.site.cleared"),
        Clearing::Failed => tr!("browser.site.clear-failed"),
    };
    let settings = origin.clone();
    column! {
        text(host).label().semibold().no_wrap();
        row! {
            icon(symbol).extent(Length::px(18.0)).color(tint);
            text(connection).grow(1.0).min_width(0.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        text(origin).caption().muted().no_wrap().min_width(0.0);
        row! {
            text(summary).grow(1.0).min_width(0.0);
            button(text(tr!("browser.site.delete")))
                .secondary()
                .small()
                .focusless()
                .enabled(clearing != Clearing::Busy)
                .on_press(move |shell: &mut Shell| shell.clear_site_data(tab));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        row! {
            button(text(tr!("browser.site.settings")))
                .ghost()
                .small()
                .focusless()
                .on_press(move |shell: &mut Shell| shell.open_site_settings(&settings));
        };
    }
    .gap(Space::NORMAL)
    .width(WIDTH)
    .any()
}
