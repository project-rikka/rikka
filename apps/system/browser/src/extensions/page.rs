use std::path::PathBuf;

use browser_pages::Cause;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_web::{Extension, ExtensionMetaData, PermissionSet};
use rikka_widgets::{column, row};

use crate::extensions::icons::icon;
use crate::shell::Shell;
use browser_pages::Internal;

const GALLERY: &str = "https://addons.mozilla.org/firefox/extensions/";
const WIDEST: f32 = 760.0;
const ICON: f32 = 32.0;
const CHANGING: &str = "rikka-browser-extension-change";
const INSTALLING: &str = "rikka-browser-extension-install";
const PACKAGES: [&str; 2] = ["*.xpi", "*.zip"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Optional {
    Permission,
    Origin,
    DataCollection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
    Enable(bool),
    Private(bool),
    Optional {
        kind: Optional,
        name: String,
        granted: bool,
    },
    Update,
    Remove,
}

fn optional_set(kind: Optional, name: String) -> PermissionSet {
    let mut set = PermissionSet::default();
    match kind {
        Optional::Permission => set.permissions.push(name),
        Optional::Origin => set.origins.push(name),
        Optional::DataCollection => set.data_collection.push(name),
    }
    set
}

fn optionals(meta: &ExtensionMetaData) -> Vec<(Optional, String, bool)> {
    [
        (
            Optional::Permission,
            &meta.optional_permissions,
            &meta.granted_optional_permissions,
        ),
        (
            Optional::Origin,
            &meta.optional_origins,
            &meta.granted_optional_origins,
        ),
        (
            Optional::DataCollection,
            &meta.optional_data_collection_permissions,
            &meta.granted_optional_data_collection_permissions,
        ),
    ]
    .into_iter()
    .flat_map(|(kind, offered, granted)| {
        offered
            .iter()
            .map(move |name| (kind, name.clone(), granted.contains(name)))
    })
    .collect()
}

impl Shell {
    fn change_extension(&mut self, id: String, change: Change) {
        let key = self.model().profile_here(self.id());
        let Some(runtime) = self.browser().running_engine(key) else {
            return;
        };
        let extension_id = id.clone();
        spawn_keyed(
            (CHANGING, id),
            async move {
                let extensions = runtime.extensions();
                match change {
                    Change::Enable(true) => extensions.enable(extension_id).await.map(Some),
                    Change::Enable(false) => extensions.disable(extension_id).await.map(Some),
                    Change::Private(allowed) => extensions
                        .allow_in_private(extension_id, allowed)
                        .await
                        .map(Some),
                    Change::Optional {
                        kind,
                        name,
                        granted: true,
                    } => extensions
                        .grant_optional(extension_id, optional_set(kind, name))
                        .await
                        .map(Some),
                    Change::Optional {
                        kind,
                        name,
                        granted: false,
                    } => extensions
                        .revoke_optional(extension_id, optional_set(kind, name))
                        .await
                        .map(Some),
                    Change::Update => extensions.update(extension_id).await,
                    Change::Remove => extensions.uninstall(extension_id).await.map(|()| None),
                }
            },
            move |shell: &mut Shell, changed: rikka_web::Result<Option<Extension>>| {
                if let Err(error) = changed {
                    tracing::warn!(%error, "failed to update extension");
                }
                shell.list_extensions();
            },
        );
    }

    fn open_options(&mut self, url: String) {
        self.open_aside_focused(url);
    }

    fn choose_extension_file(&mut self) {
        let packages = PACKAGES.into_iter().fold(
            Filter::named(tr!("browser.extensions.packages").to_string()),
            Filter::pattern,
        );
        let chooser = Chooser::open(tr!("browser.extensions.choose-file").to_string())
            .parent(window_identifier())
            .filter(packages);
        spawn_blocking(
            move || chooser.run(),
            |shell: &mut Shell, chosen: rikka_dialog::Result<ChooserResponse>| match chosen {
                Ok(ChooserResponse::Files(files)) => {
                    if let Some(path) = files.into_iter().next() {
                        shell.install_extension_file(path);
                    }
                }
                Ok(ChooserResponse::Cancelled) => {}
                Err(error) => tracing::warn!(%error, "failed to open file chooser"),
            },
        );
    }

    fn install_extension_file(&mut self, path: PathBuf) {
        let key = self.model().profile_here(self.id());
        let runtime = match self.browser().engine(key) {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(%error, "failed to install extension");
                return;
            }
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.browser().extensions().fail(None);
        spawn_keyed(
            (INSTALLING, path.clone()),
            async move { runtime.extensions().install_file(path).await },
            move |shell: &mut Shell, installed: rikka_web::Result<Extension>| match installed {
                Ok(_) => shell.list_extensions(),
                Err(error) => {
                    tracing::warn!(%error, "failed to install extension");
                    shell.browser().extensions().fail(Some(name.clone()));
                }
            },
        );
    }
}

fn card(extension: &Extension) -> Element {
    let meta = &extension.meta_data;
    let id = extension.web_extension_id.clone();
    let name = meta.name.clone().unwrap_or_else(|| id.clone());
    let version = meta.version.clone().unwrap_or_default();
    let description = meta
        .description
        .clone()
        .filter(|description| !description.is_empty());
    let options = meta.options_page_url.clone().map(|url| {
        button(text(tr!("browser.extensions.options")))
            .secondary()
            .small()
            .on_press(move |shell: &mut Shell| shell.open_options(url.clone()))
    });
    let (enabled, private) = (meta.enabled, meta.private_browsing_allowed);
    let offered = optionals(meta);
    let optional = (!offered.is_empty()).then(|| {
        let rows = offered.into_iter().map(|(kind, name, granted)| {
            let extension = id.clone();
            let label = name.clone();
            checkbox(granted)
                .label(text(label).caption())
                .on_toggle(move |shell: &mut Shell, on: bool| {
                    let change = Change::Optional {
                        kind,
                        name: name.clone(),
                        granted: on,
                    };
                    shell.change_extension(extension.clone(), change);
                })
                .any()
        });
        column! {
            text(tr!("browser.extensions.optional")).caption().muted();
            column_of(rows).gap(Space::HAIR).fill_width();
        }
        .gap(Space::TIGHT)
        .fill_width()
    });
    let (toggled, privately, updated, removed) = (id.clone(), id.clone(), id.clone(), id);
    container(
        column! {
            row! {
                icon(meta.icons.as_ref(), ICON);
                column! {
                    text(name).label().semibold().no_wrap();
                    (!version.is_empty()).then(|| text(version).caption().muted());
                }
                .gap(Space::HAIR)
                .grow(1.0)
                .min_width(0.0);
                switch(enabled).on_toggle(move |shell: &mut Shell, on: bool| {
                    shell.change_extension(toggled.clone(), Change::Enable(on))
                });
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width();
            description.map(|description| text(description).caption().muted().fill_width());
            checkbox(private)
                .label(text(tr!("browser.extensions.private")).caption())
                .on_toggle(move |shell: &mut Shell, on: bool| {
                    shell.change_extension(privately.clone(), Change::Private(on))
                });
            optional;
            row! {
                options;
                button(text(tr!("browser.extensions.update")))
                    .secondary()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.change_extension(updated.clone(), Change::Update));
                spacer().grow(1.0);
                button(text(tr!("browser.extensions.remove")))
                    .danger()
                    .small()
                    .on_press(move |shell: &mut Shell| shell.change_extension(removed.clone(), Change::Remove));
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

fn gallery() -> impl View {
    button(text(tr!("browser.extensions.find")))
        .primary()
        .on_press(|shell: &mut Shell| shell.open_aside_focused(GALLERY.to_owned()))
}

fn from_file() -> impl View {
    button(text(tr!("browser.extensions.install-file")))
        .secondary()
        .on_press(|shell: &mut Shell| shell.choose_extension_file())
}

pub(crate) fn page(shell: &Shell) -> Element {
    let key = shell.model().profile_here(shell.id());
    let extensions = shell.browser().extensions();
    let installed: Vec<Extension> = extensions
        .installed(key)
        .into_iter()
        .filter(|extension| !extension.is_built_in)
        .collect();
    let failed = extensions.state().failed.clone();
    let listed = match installed.is_empty() {
        true => text(tr!("browser.extensions.none")).muted().any(),
        false => column_of(
            installed
                .iter()
                .map(|extension| card(extension).keyed(extension.web_extension_id.clone())),
        )
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
    };
    scrollable(
        column! {
            row! {
                text(tr!("browser.extensions.title")).title().grow(1.0).min_width(0.0);
                from_file();
                gallery();
            }
            .gap(Space::NORMAL)
            .align_items(Align::Center)
            .fill_width();
            text(tr!("browser.extensions.explained")).caption().muted().fill_width();
            failed.map(|name| {
                text(tr!("browser.extensions.failed", name = &name))
                    .caption()
                    .color(palette().danger)
                    .fill_width()
            });
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
    let key = shell.model().profile_here(shell.id());
    let count = shell
        .browser()
        .extensions()
        .installed(key)
        .iter()
        .filter(|extension| !extension.is_built_in)
        .count();
    rikka_widgets::page([section_of(count)]).any()
}

fn section_of(count: usize) -> impl View {
    rikka_widgets::section(
        tr!("browser.extensions.title"),
        column! {
            text(tr!("browser.extensions.installed", count = count as i64)).fill_width();
            row! {
                button(text(tr!("browser.extensions.manage")))
                    .secondary()
                    .on_press(|shell: &mut Shell| {
                        shell.go(Internal::Extensions.address(), Cause::Typed)
                    });
                from_file();
                gallery();
            }
            .gap(Space::SNUG);
        }
        .gap(Space::NORMAL)
        .fill_width(),
    )
}
