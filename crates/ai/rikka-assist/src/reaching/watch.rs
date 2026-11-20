use std::ffi::OsStr;
use std::fs::DirBuilder;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Weak};
use std::thread;

use rikka_seam::net::SocketClient;
use rikka_seam::{Delivery, EventQueue, Request, SeamClient, SeamError};
use rikka_watch::{Change, ChangeKind, Event, Reach, Watcher};

use super::Catalogue;
use crate::protocol::{
    CAPABILITY, CHANGED, DESCRIBE, Described, SOCKET_EXTENSION, VERSION, folder,
};

#[cfg(unix)]
const PRIVATE: u32 = 0o700;

pub(super) fn start(catalogue: &Arc<Catalogue>) {
    match folder() {
        Some(folder) => start_in(catalogue, folder),
        None => tracing::info!("no desktop session, app tools are unavailable"),
    }
}

pub(super) fn start_in(catalogue: &Arc<Catalogue>, folder: PathBuf) {
    if let Err(error) = private_folder().create(&folder) {
        return tracing::warn!(
            "failed to create app tools directory {}: {error}",
            folder.display()
        );
    }

    let (sender, appeared) = mpsc::channel();
    let watcher = Watcher::spawn(move |event| {
        if let Event::Changed(Change {
            path,
            kind: ChangeKind::Created,
        }) = event
        {
            let _ = sender.send(path);
        }
    });
    let watcher = match watcher {
        Ok(watcher) => watcher,
        Err(error) => return tracing::warn!("failed to scan for app tools: {error}"),
    };
    if let Err(error) = watcher.watch(&folder, Reach::Entries) {
        return tracing::warn!("failed to watch {} for apps: {error}", folder.display());
    }
    *catalogue.watcher.lock() = Some(watcher);

    let catalogue = Arc::downgrade(catalogue);
    let started = thread::Builder::new()
        .name("rikka-assist-apps".to_owned())
        .spawn(move || watching(&catalogue, &folder, &appeared));
    if let Err(error) = started {
        tracing::warn!("failed to scan for app tools: {error}");
    }
}

fn watching(catalogue: &Weak<Catalogue>, folder: &Path, appeared: &Receiver<PathBuf>) {
    for path in offered_in(folder) {
        reach(catalogue, path);
    }
    for path in appeared.iter().filter(|path| offers(path)) {
        reach(catalogue, path);
    }
}

fn offered_in(folder: &Path) -> Vec<PathBuf> {
    let mut sockets: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| offers(path))
        .collect();
    sockets.sort();
    sockets
}

fn offers(path: &Path) -> bool {
    path.extension() == Some(OsStr::new(SOCKET_EXTENSION))
}

fn reach(catalogue: &Weak<Catalogue>, path: PathBuf) {
    let Some(live) = catalogue.upgrade() else {
        return;
    };
    if !live.begins_reaching(&path) {
        return;
    }
    let catalogue = catalogue.clone();
    let started = thread::Builder::new()
        .name("rikka-assist-app".to_owned())
        .spawn(move || attend(&catalogue, &path));
    if let Err(error) = started {
        tracing::warn!("app tools socket is unreachable: {error}");
    }
}

fn attend(catalogue: &Weak<Catalogue>, path: &Path) {
    if let Some(socket) = connected(path) {
        let client = socket.client();
        let mut events = socket.events();
        loop {
            let described = match described(&client) {
                Ok(described) => described,
                Err(error) => {
                    tracing::debug!("failed to read manifest from {}: {error}", path.display());
                    break;
                }
            };
            let Some(live) = catalogue.upgrade() else {
                return;
            };
            live.arrived(path, described, &client);
            drop(live);
            if !changed(&mut events) {
                break;
            }
        }
    }
    if let Some(live) = catalogue.upgrade() {
        live.left(path);
    }
}

#[cfg(unix)]
fn private_folder() -> DirBuilder {
    use std::os::unix::fs::DirBuilderExt as _;
    let mut builder = DirBuilder::new();
    builder.recursive(true).mode(PRIVATE);
    builder
}

#[cfg(windows)]
fn private_folder() -> DirBuilder {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    builder
}

fn connected(path: &Path) -> Option<SocketClient> {
    SocketClient::connect(path)
        .inspect_err(|error| tracing::debug!(%error, path = %path.display(), "app tools socket is unreachable"))
        .ok()
}

fn described(client: &SeamClient) -> Result<Described, SeamError> {
    client
        .call(Request::new(CAPABILITY, DESCRIBE).require(VERSION))
        .blocking_wait()?
        .decode()
}

fn changed(events: &mut EventQueue) -> bool {
    loop {
        match events.blocking_next() {
            None => return false,
            Some(Delivery::Lagged(_)) => return true,
            Some(Delivery::Event(event))
                if event.capability().as_str() == CAPABILITY && event.name() == CHANGED =>
            {
                return true;
            }
            Some(Delivery::Event(_)) => {}
        }
    }
}
