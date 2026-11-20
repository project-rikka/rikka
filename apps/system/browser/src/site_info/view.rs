use browser_store::Decision;
use rikka_app::prelude::*;
use rikka_format::format_bytes;
use rikka_web::Partition;
use rikka_widgets::{column, row};

use crate::shell::Shell;
use crate::site_info::{BADGE, Capture, Clearing, PEEK_BADGE};
use browser_model::{Ask, SITE_ASKS};
use browser_model::{Capturing, ProfileKey, Security, TabId};
use browser_pages::Certificate;
use browser_pages::{Internal, Section};

const GLYPH: f32 = 15.0;
const SITE_DATA: &str = "rikka-browser-site-data";
const WIDTH: f32 = 340.0;
const CHOICE: f32 = 120.0;

pub(crate) fn security_glyph(security: &Security) -> (MaterialSymbol, Color) {
    match security {
        Security::Secure(_) => (MaterialSymbol::LOCK, palette().text_muted),
        Security::Broken { .. } => (MaterialSymbol::WARNING, palette().danger),
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

pub(crate) fn peek_badge(tab: TabId, security: &Security, size: f32) -> Element {
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

pub(crate) fn capturing(capture: Capturing) -> Option<Element> {
    let devices: Vec<MaterialSymbol> = [
        (capture.screen, MaterialSymbol::SCREEN_SHARE),
        (capture.camera, MaterialSymbol::VIDEOCAM),
        (capture.microphone, MaterialSymbol::MIC),
    ]
    .into_iter()
    .filter_map(|(on, symbol)| on.then_some(symbol))
    .collect();
    if devices.is_empty() {
        return None;
    }
    let glyphs = row_of(devices.into_iter().map(|symbol| {
        icon(symbol)
            .extent(Length::px(GLYPH))
            .color(palette().danger)
            .any()
    }))
    .gap(Space::HAIR);
    Some(
        button(glyphs)
            .ghost()
            .small()
            .focusless()
            .on_press(|shell: &mut Shell| shell.open_site_info())
            .any(),
    )
}

pub(crate) fn ask_name(ask: Ask) -> Label {
    match ask {
        Ask::Location => tr!("browser.site.location"),
        Ask::Camera => tr!("browser.site.camera"),
        Ask::Microphone => tr!("browser.site.microphone"),
        Ask::Notifications => tr!("browser.site.notifications"),
        Ask::Screen => tr!("browser.site.screen"),
        Ask::ProtectedMedia => tr!("browser.site.protected-media"),
        Ask::StorageAccess => tr!("browser.site.storage-access"),
    }
}

pub(crate) fn decision_choices() -> [Label; 3] {
    [
        tr!("browser.site.ask"),
        tr!("browser.site.allow"),
        tr!("browser.site.block"),
    ]
}

pub(crate) fn decision_at(at: usize) -> Option<Decision> {
    match at {
        1 => Some(Decision::Allow),
        2 => Some(Decision::Deny),
        _ => None,
    }
}

pub(crate) fn decision_index(decision: Option<Decision>) -> usize {
    match decision {
        None => 0,
        Some(Decision::Allow) => 1,
        Some(Decision::Deny) => 2,
    }
}

pub(crate) fn view(shell: &Shell, tab: TabId) -> Element {
    let model = shell.model();
    let Some(tab_entry) = model.tab(tab) else {
        return Element::empty();
    };
    let url = (*tab_entry.meta.url).clone();
    let security = (*tab_entry.meta.security).clone();
    let capture = *tab_entry.meta.capture;
    let zoom = *tab_entry.meta.zoom;
    let persistent = tab_entry.profile != ProfileKey::Private;
    drop(model);
    let host = crate::address::display_address(&url).host;
    let clearing = shell.overlays.site.clearing();
    column! {
        text(host).label().semibold().no_wrap();
        connection(&security, shell.overlays.site.certificate_open());
        capture_rows(tab, capture);
        permissions(shell, tab);
        data_row(shell, tab, clearing);
        ((zoom - 1.0).abs() > 0.001).then(|| zoom_row(zoom));
        persistent.then(settings_link);
    }
    .gap(Space::NORMAL)
    .width(WIDTH)
    .any()
}

fn connection(security: &Security, open: bool) -> Element {
    let (symbol, tint, label) = match security {
        Security::Secure(_) => (
            MaterialSymbol::LOCK,
            palette().text_muted,
            tr!("browser.site.secure"),
        ),
        Security::Broken { .. } => (
            MaterialSymbol::WARNING,
            palette().danger,
            tr!("browser.site.broken"),
        ),
        Security::Insecure => (
            MaterialSymbol::LOCK_OPEN,
            palette().warning,
            tr!("browser.site.insecure"),
        ),
        Security::Unknown => (
            MaterialSymbol::INFO,
            palette().text_muted,
            tr!("browser.site.unknown"),
        ),
    };
    let certificate = match security {
        Security::Secure(certificate) | Security::Broken { certificate, .. } => {
            certificate.as_ref()
        }
        Security::Insecure | Security::Unknown => None,
    };
    let problems: Vec<Label> = match security {
        Security::Broken {
            mixed, exception, ..
        } => [
            (*mixed, tr!("browser.site.mixed")),
            (*exception, tr!("browser.site.exception")),
        ]
        .into_iter()
        .filter_map(|(present, label)| present.then_some(label))
        .collect(),
        _ => Vec::new(),
    };
    let toggle = certificate.map(|_| {
        button(
            icon(match open {
                true => MaterialSymbol::EXPAND_LESS,
                false => MaterialSymbol::EXPAND_MORE,
            })
            .extent(Length::px(18.0)),
        )
        .ghost()
        .icon_only()
        .small()
        .focusless()
        .on_press(|shell: &mut Shell| shell.overlays.site.toggle_certificate())
    });
    column! {
        row! {
            icon(symbol).extent(Length::px(18.0)).color(tint);
            text(label).grow(1.0).min_width(0.0);
            toggle;
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        column_of(
            problems
                .into_iter()
                .map(|problem| text(problem).caption().color(palette().danger).any()),
        )
        .gap(Space::HAIR);
        certificate.filter(|_| open).map(details);
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn date(seconds: Option<i64>) -> String {
    seconds
        .and_then(|seconds| jiff::Timestamp::from_second(seconds).ok())
        .map(|at| {
            let date = at.to_zoned(jiff::tz::TimeZone::system()).date();
            crate::internal::days::day(date, crate::internal::days::today())
        })
        .unwrap_or_default()
}

pub(crate) fn details(certificate: &Certificate) -> Element {
    let line = |label: Label, value: String| {
        row! {
            text(label).caption().muted().width(96.0);
            text(value).caption().grow(1.0).min_width(0.0);
        }
        .gap(Space::SNUG)
        .fill_width()
        .any()
    };
    let pem = certificate.pem.clone();
    column! {
        line(tr!("browser.certificate.subject"), certificate.subject.clone().unwrap_or_default());
        line(tr!("browser.certificate.issuer"), certificate.issuer.clone().unwrap_or_default());
        line(tr!("browser.certificate.valid-from"), date(certificate.valid_from));
        line(tr!("browser.certificate.valid-until"), date(certificate.valid_until));
        row! {
            button(text(tr!("browser.certificate.copy")))
                .secondary()
                .small()
                .focusless()
                .on_press(move || rikka_ui::clipboard::set_text(&pem));
        };
    }
    .gap(Space::TIGHT)
    .padding(Inset::all(Space::SNUG))
    .fill_width()
    .any()
}

fn capture_rows(tab: TabId, capture: Capturing) -> Option<Element> {
    let devices = match (capture.camera, capture.microphone) {
        (true, true) => Some(tr!("browser.site.using-camera-microphone")),
        (true, false) => Some(tr!("browser.site.using-camera")),
        (false, true) => Some(tr!("browser.site.using-microphone")),
        (false, false) => None,
    };
    let rows: Vec<Element> = [
        (
            capture.screen.then(|| tr!("browser.site.sharing-screen")),
            tr!("browser.site.stop-sharing"),
            Capture::Screen,
        ),
        (devices, tr!("browser.site.stop-capture"), Capture::Devices),
    ]
    .into_iter()
    .filter_map(|(label, stop, what)| {
        let label = label?;
        Some(
            row! {
                text(label).color(palette().danger).grow(1.0).min_width(0.0);
                button(text(stop))
                    .secondary()
                    .small()
                    .focusless()
                    .on_press(move |shell: &mut Shell| shell.stop_capture(tab, what));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width()
            .any(),
        )
    })
    .collect();
    (!rows.is_empty()).then(|| column_of(rows).gap(Space::SNUG).fill_width().any())
}

fn permissions(shell: &Shell, tab: TabId) -> Element {
    let decided_only = [Ask::ProtectedMedia]
        .into_iter()
        .filter(|ask| shell.site_permission(tab, *ask).is_some());
    let rows = SITE_ASKS.into_iter().chain(decided_only).map(|ask| {
        let chosen = decision_index(shell.site_permission(tab, ask));
        row! {
            text(ask_name(ask)).grow(1.0).min_width(0.0);
            combo(Some(chosen))
                .options(decision_choices())
                .on_select(move |shell: &mut Shell, at: usize| {
                    shell.set_site_permission(tab, ask, decision_at(at));
                })
                .width(CHOICE);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width()
        .any()
    });
    column! {
        text(tr!("browser.site.permissions")).caption().muted();
        column_of(rows).gap(Space::TIGHT).fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn stored(shell: &Shell, tab: TabId) -> Label {
    let Some((key, url)) = shell
        .model()
        .tab(tab)
        .map(|tab_entry| (tab_entry.profile, (*tab_entry.meta.url).clone()))
    else {
        return tr!("browser.site.data-kept");
    };
    let (Some(site), Some(runtime)) = (
        browser_pages::registrable_site(&url),
        shell.browser().running_engine(key),
    ) else {
        return tr!("browser.site.data-kept");
    };
    let storage = runtime.storage();
    let partition = Partition {
        context: None,
        private: runtime.is_ephemeral(),
    };
    let asked = site.clone();
    match fetch((SITE_DATA, key, site), move || async move {
        storage.site_data(asked, &partition).await
    }) {
        Fetch::Ready(data) if data.cookies == 0 && data.usage == 0 => {
            tr!("browser.site.no-data")
        }
        Fetch::Ready(data) => tr!(
            "browser.site.data-amount",
            cookies = i64::from(data.cookies),
            size = &format_bytes(data.usage)
        ),
        Fetch::Loading | Fetch::Failed(_) => tr!("browser.site.data-kept"),
    }
}

fn data_row(shell: &Shell, tab: TabId, clearing: Clearing) -> Element {
    let summary = match clearing {
        Clearing::Idle => stored(shell, tab),
        Clearing::Busy => tr!("browser.site.clearing"),
        Clearing::Done => tr!("browser.site.cleared"),
    };
    column! {
        text(tr!("browser.site.data")).caption().muted();
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
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn zoom_row(zoom: f64) -> Element {
    row! {
        text(tr!("browser.site.zoom", percent = &format!("{:.0}", zoom * 100.0)))
            .grow(1.0)
            .min_width(0.0);
        button(text(tr!("browser.site.reset")))
            .secondary()
            .small()
            .focusless()
            .on_press(|shell: &mut Shell| shell.zoom(0));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn settings_link() -> Element {
    row! {
        button(text(tr!("browser.site.settings")))
            .ghost()
            .small()
            .focusless()
            .on_press(|shell: &mut Shell| {
                shell.overlays.close_popup();
                shell.open_internal(Internal::Settings(Some(Section::Sites)));
            });
    }
    .any()
}
