use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use zbus::interface;
use zbus::object_server::{ObjectServer, SignalEmitter};
use zbus::zvariant::OwnedObjectPath;

use crate::backend::cast::Cast;
use crate::backend::options::SourceSelection;

#[derive(Debug, Default)]
pub(crate) struct Sessions(Mutex<HashMap<OwnedObjectPath, SessionEntry>>);

#[derive(Debug, Default)]
struct SessionEntry {
    selection: Option<SourceSelection>,
    cast: Option<Cast>,
}

impl Sessions {
    pub(crate) fn insert(&self, session: &OwnedObjectPath) {
        self.entries()
            .insert(session.clone(), SessionEntry::default());
    }

    pub(crate) fn select(&self, session: &OwnedObjectPath, selection: SourceSelection) -> bool {
        match self.entries().get_mut(session) {
            Some(entry) => {
                entry.selection = Some(selection);
                true
            }
            None => false,
        }
    }

    pub(crate) fn selection(&self, session: &OwnedObjectPath) -> Option<SourceSelection> {
        self.entries()
            .get(session)
            .and_then(|entry| entry.selection.clone())
    }

    pub(crate) fn attach_cast(&self, session: &OwnedObjectPath, cast: Cast) -> bool {
        let replaced = {
            let mut entries = self.entries();
            match entries.get_mut(session) {
                Some(entry) => entry.cast.replace(cast),
                None => return false,
            }
        };
        drop(replaced);
        true
    }

    pub(crate) fn close(&self, session: &OwnedObjectPath) {
        let closed = self.entries().remove(session);
        drop(closed);
    }

    fn entries(&self) -> rikka_sync::MutexGuard<'_, HashMap<OwnedObjectPath, SessionEntry>> {
        self.0.lock()
    }
}

#[derive(Debug)]
pub(crate) struct Session {
    path: OwnedObjectPath,
    sessions: Arc<Sessions>,
}

impl Session {
    pub(crate) fn new(path: OwnedObjectPath, sessions: Arc<Sessions>) -> Session {
        Session { path, sessions }
    }
}

#[interface(name = "org.freedesktop.impl.portal.Session")]
impl Session {
    async fn close(&self, #[zbus(object_server)] server: &ObjectServer) {
        self.sessions.close(&self.path);
        if let Err(error) = server.remove::<Session, _>(&self.path).await {
            eprintln!(
                "rikka-portal-cast: cannot unexport session {}: {error}",
                self.path
            );
        }
    }

    #[zbus(signal)]
    async fn closed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
