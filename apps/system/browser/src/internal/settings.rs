use browser_pages::Cause;
use rikka_app::prelude::*;
use rikka_types::Choice;
use rikka_widgets::{column, row};

use crate::menus::profile_name;
use crate::preferences::{ArchiveAfter, HistoryKept, ZOOMS};
use crate::shell::Shell;
use crate::sidebar::Renaming;
use browser_model::ProfileId;
use browser_pages::Internal;
use browser_pages::Section;

const NAV: f32 = 220.0;
const WIDEST: f32 = 720.0;
const CHOICE: f32 = 220.0;

pub(crate) fn view(section: Option<Section>) -> Element {
    let shown = section.unwrap_or(Section::LISTED[0]);
    let content = match shown {
        Section::Appearance => component(|shell: &Shell| shell, appearance),
        Section::Tabs => component(|shell: &Shell| shell, tabs),
        Section::Home => component(|shell: &Shell| shell, crate::home::settings_view),
        Section::Profiles => component(|shell: &Shell| shell, profiles),
        Section::Privacy => component(|shell: &Shell| shell, privacy),
        Section::Permissions => component(|shell: &Shell| shell, crate::sections::permissions_view),
        Section::Content => component(|shell: &Shell| shell, crate::sections::content_view),
        Section::Network => component(|shell: &Shell| shell, crate::sections::network_view),
        Section::Zoom => component(|shell: &Shell| shell, zoom),
        Section::Performance => {
            component(|shell: &Shell| shell, crate::memory_saver::settings_view)
        }
        Section::Search => component(|shell: &Shell| shell, crate::sections::search_view),
        Section::DefaultBrowser => {
            component(|shell: &Shell| shell, crate::sections::default_browser_view)
        }
        Section::Translation => component(|shell: &Shell| shell, crate::sections::translation_view),
        Section::Extensions => component(|shell: &Shell| shell, crate::extensions::section),
        Section::HttpsOnly => component(|shell: &Shell| shell, crate::sections::https_view),
        Section::Sites => component(|shell: &Shell| shell, crate::internal::sites::view),
        Section::Downloads => {
            component(|shell: &Shell| shell, crate::internal::downloads::settings)
        }
        Section::ClearData => Element::empty(),
    };
    framed(shown, content)
}

pub(crate) fn framed(shown: Section, content: Element) -> Element {
    let lit = shown.listed_under();
    let nav = column_of(Section::LISTED.map(|section| destination(section, section == lit)))
        .gap(Space::HAIR)
        .padding(Inset::all(Space::NORMAL))
        .width(NAV)
        .fill_height();
    row! {
        nav;
        container(content.max_width(WIDEST).fill())
            .align_x(Align::Center)
            .grow(1.0)
            .fill_height();
    }
    .fill()
    .any()
}

fn destination(section: Section, chosen: bool) -> Element {
    let (symbol, title) = match section {
        Section::Appearance => (MaterialSymbol::PALETTE, tr!("browser.settings.appearance")),
        Section::Tabs => (MaterialSymbol::TAB, tr!("browser.settings.tabs")),
        Section::Home => (MaterialSymbol::HOME, tr!("browser.home.title")),
        Section::Profiles => (MaterialSymbol::PERSON, tr!("browser.settings.profiles")),
        Section::Privacy => (MaterialSymbol::PRIVACY_TIP, tr!("browser.settings.privacy")),
        Section::Permissions => (
            MaterialSymbol::PERSON_SHIELD,
            tr!("browser.protection.permissions"),
        ),
        Section::Content => (MaterialSymbol::TEXT_FORMAT, tr!("browser.content.title")),
        Section::Network => (MaterialSymbol::LAN, tr!("browser.network.title")),
        Section::Zoom => (MaterialSymbol::ZOOM_IN, tr!("browser.settings.zoom")),
        Section::Performance => (MaterialSymbol::SPEED, tr!("browser.memory.section")),
        Section::Search => (MaterialSymbol::SEARCH, tr!("browser.search.title")),
        Section::DefaultBrowser => (
            MaterialSymbol::OPEN_IN_BROWSER,
            tr!("browser.default.title"),
        ),
        Section::Translation => (MaterialSymbol::TRANSLATE, tr!("browser.translate.title")),
        Section::Extensions => (MaterialSymbol::EXTENSION, tr!("browser.extensions.title")),
        Section::HttpsOnly => (MaterialSymbol::LOCK, tr!("browser.https.title")),
        Section::Sites => (MaterialSymbol::TUNE, tr!("browser.sites.title")),
        Section::Downloads => (MaterialSymbol::DOWNLOAD, tr!("browser.downloads.settings")),
        Section::ClearData => (MaterialSymbol::DELETE_SWEEP, tr!("browser.clear.title")),
    };
    let tint = match chosen {
        true => palette().accent,
        false => palette().text,
    };
    container(
        row! {
            icon(symbol).extent(Length::px(20.0)).color(tint);
            text(title).label().medium().color(tint).no_wrap();
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::NORMAL, Space::SNUG))
    .radius(Corner::Panel)
    .background(match chosen {
        true => palette().accent.with_alpha(0.14),
        false => Color::TRANSPARENT,
    })
    .on_press(move |shell: &mut Shell| {
        shell.go(Internal::Settings(Some(section)).address(), Cause::Typed)
    })
    .fill_width()
    .any()
}

fn appearance(shell: &Shell) -> Element {
    let rounded = shell.browser().preferences().rounded_corners();
    page([section(
        tr!("browser.settings.appearance"),
        field(
            tr!("browser.settings.rounded-corners"),
            switch(rounded).on_toggle(|shell: &mut Shell, on: bool| {
                shell.browser().preferences().set_rounded_corners(on);
            }),
        ),
    )])
    .any()
}

fn tabs(shell: &Shell) -> Element {
    let chosen = shell.browser().preferences().archive_after();
    page([section(
        tr!("browser.settings.tabs"),
        column! {
            field(
                tr!("browser.settings.archive-after"),
                combo(ArchiveAfter::ALL.iter().position(|after| *after == chosen))
                    .options(ArchiveAfter::ALL.iter().copied().map(archive_after_name))
                    .on_select(|shell: &mut Shell, at: usize| {
                        if let Some(after) = ArchiveAfter::ALL.get(at) {
                            shell.browser().preferences().set_archive_after(*after);
                            shell.archive_idle();
                        }
                    })
                    .width(CHOICE),
            );
            row! {
                button(text(tr!("browser.settings.open-archive")))
                    .secondary()
                    .small()
                    .on_press(|shell: &mut Shell| shell.open_internal(Internal::Archive));
            };
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )])
    .any()
}

fn archive_after_name(after: ArchiveAfter) -> Label {
    match after {
        ArchiveAfter::Never => tr!("browser.settings.archive-never"),
        ArchiveAfter::HalfDay => tr!("browser.settings.archive-12h"),
        ArchiveAfter::Day => tr!("browser.settings.archive-24h"),
        ArchiveAfter::Week => tr!("browser.settings.archive-7d"),
        ArchiveAfter::Month => tr!("browser.settings.archive-30d"),
    }
}

fn privacy(shell: &Shell) -> Element {
    let retention = shell.browser().preferences().history_kept();
    page([crate::sections::protection_view(shell), section(
        tr!("browser.settings.privacy"),
        column! {
            field(
                tr!("browser.settings.history-kept"),
                combo(HistoryKept::ALL.iter().position(|candidate| *candidate == retention))
                    .options(HistoryKept::ALL.iter().copied().map(history_kept_name))
                    .on_select(|shell: &mut Shell, at: usize| {
                        if let Some(retention) = HistoryKept::ALL.get(at) {
                            shell.browser().preferences().set_history_kept(*retention);
                            shell.prune_history();
                        }
                    })
                    .width(CHOICE),
            );
            row! {
                button(text(tr!("browser.clear.title")))
                    .secondary()
                    .small()
                    .on_press(|shell: &mut Shell| {
                        shell.go(Internal::Settings(Some(Section::ClearData)).address(), Cause::Typed)
                    });
            };
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
    .any()])
    .any()
}

fn history_kept_name(retention: HistoryKept) -> Label {
    match retention {
        HistoryKept::Forever => tr!("browser.settings.history-forever"),
        HistoryKept::Year => tr!("browser.settings.history-year"),
        HistoryKept::Quarter => tr!("browser.settings.history-90d"),
        HistoryKept::Month => tr!("browser.settings.history-30d"),
    }
}

fn zoom(shell: &Shell) -> Element {
    let chosen = (shell.browser().preferences().default_zoom() * 100.0).round() as i64;
    page([section(
        tr!("browser.settings.zoom"),
        field(
            tr!("browser.settings.default-zoom"),
            combo(ZOOMS.iter().position(|percent| *percent == chosen))
                .options(ZOOMS.map(|percent| format!("{percent}%")))
                .on_select(|shell: &mut Shell, at: usize| {
                    if let Some(percent) = ZOOMS.get(at) {
                        shell.browser().preferences().set_default_zoom(*percent);
                        shell.refit_zoom();
                    }
                })
                .width(CHOICE),
        ),
    )])
    .any()
}

fn profiles(shell: &Shell) -> Element {
    let model = shell.model();
    let renaming = shell.sidebar().renaming();
    let rows = model.profiles.list().iter().map(|profile| {
        let id = profile.id;
        let name = profile_name(&profile.name);
        let control = match renaming == Some(Renaming::Profile(id)) {
            true => crate::sidebar::rename_field(shell),
            false => row! {
                rename_button(id, name.clone());
                button(text(tr!("browser.settings.delete-profile")))
                    .ghost()
                    .small()
                    .enabled(model.removable_profile(id))
                    .on_press(move |shell: &mut Shell| shell.delete_profile(id));
            }
            .gap(Space::TIGHT)
            .any(),
        };
        field(name, control).build().keyed(id)
    });
    page([section(
        tr!("browser.settings.profiles"),
        column! {
            column_of(rows).gap(Space::TIGHT).fill_width();
            row! {
                button(text(tr!("browser.settings.add-profile")))
                    .secondary()
                    .small()
                    .on_press(|shell: &mut Shell| shell.new_profile());
            };
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )])
    .any()
}

fn rename_button(id: ProfileId, name: String) -> Element {
    button(text(tr!("browser.menu.rename")))
        .ghost()
        .small()
        .on_press(move |shell: &mut Shell| shell.rename(Renaming::Profile(id), &name))
        .any()
}
