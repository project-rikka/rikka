use std::sync::Arc;
use std::time::Duration;

use files_core::{Entry, FolderKind, Location};
use files_vfs::{Change, ListOptions, Watch};
use rikka_app::prelude::*;
use rikka_file_operations::Store;
use rikka_ui::{IntoHandler, State, spawn_blocking};

use crate::services::Services;
use crate::tab::{Listed, Tab};
use crate::wording;

pub(crate) const CHANGE_DEBOUNCE: Duration = Duration::from_millis(400);

pub(crate) fn home() -> Location {
    rikka_dirs::home().map_or_else(|| Location::from_path("/"), Location::from_path)
}

pub(crate) fn typed(services: &Services, text: &str) -> Result<Location, String> {
    let at = Location::parse(text).unwrap_or_else(|| Location::from_path(text.trim()));
    if services.vfs.knows(at.scheme()) {
        Ok(at)
    } else {
        Err(tr!("files.error.no-backend", scheme = at.scheme()).to_string())
    }
}

pub(crate) fn read<M>(
    services: &Services,
    at: &Location,
    show_hidden: bool,
    done: impl IntoHandler<(Listed,), (), M>,
) {
    let vfs = Arc::clone(&services.vfs);
    let at = at.clone();
    let options = ListOptions {
        show_hidden,
        ..ListOptions::default()
    };
    let named = services.folder_kind(&at);

    spawn_blocking(
        move || {
            let (entries, failure) = match vfs.list(&at, options) {
                Ok(entries) => (entries, None),
                Err(error) => (Vec::new(), Some(wording::failure(&error))),
            };
            let kind = named.unwrap_or_else(|| FolderKind::detect(&entries));
            Listed {
                at,
                entries,
                failure,
                kind,
            }
        },
        done,
    );
}

pub(crate) fn enter_folder<S: State>(
    services: &Services,
    tab: &mut Tab,
    sender: &Sender<S>,
    changed: impl Fn(&mut S, Change) + Send + Sync + 'static,
) {
    tab.reset_for_navigation();
    let watching = watch(services, tab.at(), sender, changed);
    tab.set_watch(watching);
}

pub(crate) fn stat_entries<M>(
    services: &Services,
    at: Vec<Location>,
    done: impl IntoHandler<(Vec<Entry>,), (), M>,
) {
    let vfs = Arc::clone(&services.vfs);
    spawn_blocking(
        move || at.iter().filter_map(|at| vfs.stat(at).ok()).collect(),
        done,
    );
}

fn watch<S: State>(
    services: &Services,
    at: &Location,
    sender: &Sender<S>,
    changed: impl Fn(&mut S, Change) + Send + Sync + 'static,
) -> Option<Watch> {
    let sender = sender.clone();
    let changed = Arc::new(changed);
    let sink: files_vfs::WatchSink = Arc::new(move |change: Change| {
        let changed = Arc::clone(&changed);
        sender
            .send(move |state: &mut S| changed(state, change))
            .ok();
    });
    services
        .vfs
        .watch(at, sink)
        .inspect_err(|error| tracing::warn!(%error, %at, "cannot watch folder"))
        .ok()
}

pub(crate) fn describe(services: &Services, entry: &Entry) -> String {
    wording::kind(services.mime.describe(&entry.mime), entry.extension())
}
