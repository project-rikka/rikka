use browser_pages::{ChromePage, Internal};
use futures_util::future::Either;
use rikka_app::prelude::*;
use rikka_web::{ExtensionInfo, ExtensionKind, InstallKind};
use rikka_widgets::{column, row};

use crate::shell::Shell;
use browser_model::ProfileKey;

const STORE: &str = "https://chromewebstore.google.com/";
const WIDEST: f32 = 760.0;
const LISTING: &str = "rikka-browser-extensions";
const CHANGING: &str = "rikka-browser-extension-change";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Enable(bool),
    Remove,
}

fn listed(extension: &ExtensionInfo) -> bool {
    extension.kind == ExtensionKind::Extension
}

impl Shell {
    pub(crate) fn list_extensions(&mut self) {
        let key = self.model().profile_here(self.id());
        if let Err(error) = self.browser().engine(key) {
            tracing::warn!(%error, "failed to start Chrome for extensions");
            return;
        }
        self.list_extensions_of(key);
    }

    pub(crate) fn list_extensions_of(&mut self, key: ProfileKey) {
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        let own = runtime.extension_id().to_owned();
        spawn_keyed(
            (LISTING, key),
            runtime.extensions().installed(),
            move |shell: &mut Shell, installed: rikka_web::Result<Vec<ExtensionInfo>>| {
                match installed {
                    Ok(mut installed) => {
                        installed.retain(|extension| extension.id != own);
                        shell
                            .browser()
                            .services()
                            .chrome
                            .set_extensions(key, installed);
                    }
                    Err(error) => tracing::warn!(%error, "failed to list extensions"),
                }
            },
        );
    }

    fn change_extension(&mut self, id: String, change: Change) {
        let key = self.model().profile_here(self.id());
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        let extensions = runtime.extensions();
        let changed = match change {
            Change::Enable(enabled) => Either::Left(extensions.set_enabled(&id, enabled)),
            Change::Remove => Either::Right(extensions.uninstall(&id)),
        };
        spawn_keyed(
            (CHANGING, id),
            changed,
            move |shell: &mut Shell, changed: rikka_web::Result<()>| {
                if let Err(error) = changed {
                    tracing::warn!(%error, "failed to change extension");
                }
                shell.list_extensions_of(key);
            },
        );
    }
}

fn card(extension: &ExtensionInfo) -> Element {
    let id = extension.id.clone();
    let managed = extension.install_type == InstallKind::Admin;
    let description = Some(extension.description.clone()).filter(|text| !text.is_empty());
    let options = extension.options_url.clone().map(|url| {
        button(text(tr!("browser.extensions.options")))
            .secondary()
            .small()
            .on_press(move |shell: &mut Shell| shell.open_page(url.clone()))
    });
    let details = extension.settings_page();
    let removable = !managed;
    let (toggled, removed) = (id.clone(), id);
    container(
        column! {
            row! {
                icon(MaterialSymbol::EXTENSION).extent(Length::px(28.0)).color(palette().text_muted);
                column! {
                    text(extension.name.clone()).label().semibold().no_wrap();
                    (!extension.version.is_empty()).then(|| text(extension.version.clone()).caption().muted());
                }
                .gap(Space::HAIR)
                .grow(1.0)
                .min_width(0.0);
                switch(extension.enabled)
                    .enabled(extension.may_disable)
                    .on_toggle(move |shell: &mut Shell, on: bool| {
                        shell.change_extension(toggled.clone(), Change::Enable(on))
                    });
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width();
            description.map(|description| text(description).caption().muted().fill_width());
            managed.then(|| text(tr!("browser.extensions.managed")).caption().muted().fill_width());
            row! {
                options;
                button(text(tr!("browser.extensions.details")))
                    .ghost()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.open_page(details.clone()));
                spacer().grow(1.0);
                removable.then(|| {
                    button(text(tr!("browser.extensions.remove")))
                        .danger()
                        .small()
                        .on_press(move |shell: &mut Shell| {
                            shell.change_extension(removed.clone(), Change::Remove)
                        })
                });
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .padding(Inset::all(Space::NORMAL))
    .background(palette().surface)
    .border(1.0, palette().outline.with_alpha(0.5))
    .radius(Corner::Panel)
    .fill_width()
    .any()
}

fn store() -> impl View {
    button(text(tr!("browser.extensions.find")))
        .primary()
        .on_press(|shell: &mut Shell| shell.open_page(STORE.to_owned()))
}

fn chrome_page() -> impl View {
    button(text(tr!("browser.extensions.chrome-page")))
        .secondary()
        .on_press(|shell: &mut Shell| shell.open_page(ChromePage::Extensions.address().to_owned()))
}

fn installed(shell: &Shell) -> Option<Vec<ExtensionInfo>> {
    let key = shell.model().profile_here(shell.id());
    shell
        .browser()
        .services()
        .chrome
        .extensions(key)
        .map(|extensions| extensions.into_iter().filter(listed).collect())
}

pub(crate) fn page(shell: &Shell) -> Element {
    let listed = match installed(shell) {
        None => container(spinner().extent(Length::px(20.0)))
            .center()
            .fill_width()
            .any(),
        Some(installed) if installed.is_empty() => {
            text(tr!("browser.extensions.none")).muted().any()
        }
        Some(installed) => column_of(
            installed
                .iter()
                .map(|extension| card(extension).keyed(extension.id.clone())),
        )
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
    };
    scrollable(
        column! {
            row! {
                text(tr!("browser.extensions.title")).title().grow(1.0).min_width(0.0);
                chrome_page();
                store();
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width();
            text(tr!("browser.extensions.explained")).caption().muted().fill_width();
            listed;
        }
        .gap(Space::WIDE)
        .padding(Inset::all(Space::WIDE))
        .max_width(WIDEST)
        .fill_width(),
    )
    .fill()
    .any()
}

pub(crate) fn section(shell: &Shell) -> Element {
    let count = installed(shell).map_or(0, |installed| installed.len());
    rikka_widgets::page([rikka_widgets::section(
        tr!("browser.extensions.title"),
        column! {
            text(tr!("browser.extensions.installed", count = count as i64)).fill_width();
            row! {
                button(text(tr!("browser.extensions.manage")))
                    .secondary()
                    .on_press(|shell: &mut Shell| shell.go(Internal::Extensions.address()));
                chrome_page();
                store();
            }
            .gap(Space::SNUG);
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )])
    .any()
}
