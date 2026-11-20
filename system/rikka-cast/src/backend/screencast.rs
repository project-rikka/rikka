use std::sync::Arc;

use rikka_portal_backend::{CANCELLED, ENDED, Helper, HelperOutcome, SUCCESS};
use zbus::interface;
use zbus::object_server::ObjectServer;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::backend::cast::Cast;
use crate::backend::options::{self, SelectOptions, SourceSelection};
use crate::backend::results::{CreateSessionResults, EmptyResults, StartResults};
use crate::backend::session::{Session, Sessions};
use crate::protocol::{Answer, Persist};

const VERSION: u32 = 6;

#[derive(Debug)]
pub(crate) struct ScreenCast {
    picker: Helper,
    sessions: Arc<Sessions>,
}

impl ScreenCast {
    pub(crate) fn new(picker: Helper) -> ScreenCast {
        ScreenCast {
            picker,
            sessions: Arc::new(Sessions::default()),
        }
    }

    async fn choose_sources(
        &self,
        server: &ObjectServer,
        handle: &OwnedObjectPath,
        selection: &SourceSelection,
        app_id: String,
    ) -> Option<Answer> {
        if !selection.restore.is_empty() {
            return Some(Answer {
                sources: selection.restore.clone(),
                remember: true,
            });
        }

        match self
            .picker
            .ask(server, handle, &selection.picker_request(app_id))
            .await
        {
            HelperOutcome::Answered(answer) => Some(*answer),
            HelperOutcome::Cancelled | HelperOutcome::Ended => None,
        }
    }
}

#[interface(name = "org.freedesktop.impl.portal.ScreenCast")]
impl ScreenCast {
    async fn create_session(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        options: std::collections::HashMap<String, OwnedValue>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, CreateSessionResults) {
        drop((handle, app_id, options));

        let session = Session::new(session_handle.clone(), Arc::clone(&self.sessions));
        if let Err(error) = server.at(&session_handle, session).await {
            eprintln!("rikka-portal-cast: cannot export session {session_handle}: {error}");
            return (ENDED, CreateSessionResults::new(""));
        }
        self.sessions.insert(&session_handle);
        (SUCCESS, CreateSessionResults::new(session_handle.as_str()))
    }

    async fn select_sources(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        options: SelectOptions,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, EmptyResults) {
        drop((handle, app_id));

        let Some(selection) = options.into_selection() else {
            self.sessions.close(&session_handle);
            if let Err(error) = server.remove::<Session, _>(&session_handle).await {
                eprintln!("rikka-portal-cast: cannot unexport session {session_handle}: {error}");
            }
            return (ENDED, EmptyResults::default());
        };
        match self.sessions.select(&session_handle, selection) {
            true => (SUCCESS, EmptyResults::default()),
            false => (ENDED, EmptyResults::default()),
        }
    }

    async fn start(
        &self,
        handle: OwnedObjectPath,
        session_handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        options: std::collections::HashMap<String, OwnedValue>,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, StartResults) {
        drop((parent_window, options));

        let Some(mut selection) = self.sessions.selection(&session_handle) else {
            return (ENDED, StartResults::default());
        };
        let Some(mut answer) = self
            .choose_sources(server, &handle, &selection, app_id.clone())
            .await
        else {
            return (CANCELLED, StartResults::default());
        };

        let (cast, streams) = loop {
            match Cast::start(answer.sources.clone(), selection.cursor, app_id.clone()).await {
                Ok(started) => break started,
                Err(error) if !selection.restore.is_empty() => {
                    eprintln!("rikka-portal-cast: cannot restore sources: {error}; showing picker");
                    selection.restore.clear();
                    let chosen = self
                        .choose_sources(server, &handle, &selection, app_id.clone())
                        .await;
                    match chosen {
                        Some(chosen) => answer = chosen,
                        None => return (CANCELLED, StartResults::default()),
                    }
                }
                Err(error) => {
                    eprintln!("rikka-portal-cast: cannot start cast: {error}");
                    return (ENDED, StartResults::default());
                }
            }
        };

        let persist = match answer.remember {
            true => selection.persist,
            false => Persist::No,
        };
        let results = StartResults::new(&streams, persist);
        if !self.sessions.attach_cast(&session_handle, cast) {
            return (ENDED, StartResults::default());
        }
        (SUCCESS, results)
    }

    #[zbus(property)]
    fn available_source_types(&self) -> u32 {
        options::SOURCE_TYPES
    }

    #[zbus(property)]
    fn available_cursor_modes(&self) -> u32 {
        options::CURSOR_MODES
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        VERSION
    }
}
