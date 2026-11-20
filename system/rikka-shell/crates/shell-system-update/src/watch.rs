use std::sync::{Arc, OnceLock};

use rikka_nix::{Event, Interest, Monitor};
use rikka_seam::{Request, SeamClient};
use rikka_sync::Mutex;
use shell_kit::detached;

use crate::service::{ARRIVED, CAPABILITY};

const WATCHING: &str = "rikka-shell-system-update";

#[derive(Clone, Debug, Default)]
pub(super) struct Inbox(Arc<Shared>);

#[derive(Debug, Default)]
struct Shared {
    events: Mutex<Vec<Event>>,
    monitor: OnceLock<Monitor>,
}

impl Inbox {
    fn push(&self, event: Event) {
        self.0.events.lock().push(event);
    }

    pub(super) fn drain(&self) -> Vec<Event> {
        std::mem::take(&mut *self.0.events.lock())
    }
}

pub(super) fn watch(inbox: Inbox, seam: SeamClient) {
    detached(WATCHING, move || {
        let arriving = inbox.clone();
        let deliver = move |event: Event| {
            arriving.push(event);
            seam.notify(Request::new(CAPABILITY, ARRIVED));
        };
        let monitor = match Monitor::watch(Interest::Progress, deliver.clone()) {
            Ok(monitor) => monitor,
            Err(error) => return tracing::warn!("not following system updates: {error}"),
        };
        match monitor.running() {
            Ok(Some(run)) => deliver(Event::Started(run)),
            Ok(None) => {}
            Err(error) => tracing::warn!("failed to read the running system update: {error}"),
        }
        if inbox.0.monitor.set(monitor).is_err() {
            tracing::warn!("system updates are already followed");
        }
    });
}
