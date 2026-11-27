use std::sync::{Arc, Weak};
use std::thread::JoinHandle;

use rikka_nested::Nested;
use rikka_sync::{Mutex, MutexGuard};
use serde_json::{Value, json};

use crate::channel::{Called, Link, Listening, Rendezvous};
use crate::error::{Error, Result};
use crate::event::{CloseReason, RuntimeEvent};
use crate::ids::{SessionId, TabId, WindowId};
use crate::outbox::Outbox;
use crate::process::supervise::{Exit, Watch};
use crate::runtime::downloads::Poller;
use crate::runtime::state::{Pending, Phase, State};
use crate::threads;

pub(crate) const CLAIM_PREFIX: &str = "rikka-claim-";

pub(crate) struct Parting {
    pub(crate) watch: Watch,
    pub(crate) listening: Listening,
    pub(crate) rendezvous: Rendezvous,
    pub(crate) nested: Nested,
    pub(crate) display: Option<JoinHandle<()>>,
    pub(crate) writer: JoinHandle<()>,
}

pub(crate) struct Core {
    pub(crate) link: Arc<Link>,
    pub(crate) nested: Nested,
    pub(crate) state: Mutex<State>,
    pub(crate) outbox: Outbox<RuntimeEvent>,
    pub(crate) extension_id: String,
    pub(crate) claim_page: String,
    pub(crate) this: Weak<Core>,
    pub(crate) downloads: Poller,
    pub(crate) parting: Mutex<Option<Parting>>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Core")
            .field("extension_id", &self.extension_id)
            .finish_non_exhaustive()
    }
}

impl Core {
    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock()
    }

    pub(crate) fn call(&self, api: &str, args: Vec<Value>) -> Called {
        self.link.call(api, args)
    }

    pub(crate) fn notify(&self, api: &str, args: Vec<Value>) {
        self.link.notify(api, args);
    }

    pub(crate) fn claim_url(&self, token: &str) -> String {
        format!("{}#{token}", self.claim_page)
    }

    pub(crate) fn is_claim_url(&self, url: &str) -> bool {
        url.starts_with(&self.claim_page)
    }

    pub(crate) fn tab(&self, id: SessionId) -> Result<TabId> {
        let state = self.state();
        let entry = state.sessions.get(&id).ok_or(Error::Closed)?;
        match (entry.phase, entry.tab) {
            (Phase::Closing, _) => Err(Error::Closed),
            (_, Some(tab)) => Ok(tab),
            (_, None) => Err(Error::Rejected("the session is still opening".into())),
        }
    }

    pub(crate) fn window(&self, id: SessionId) -> Result<WindowId> {
        let state = self.state();
        let entry = state.sessions.get(&id).ok_or(Error::Closed)?;
        entry
            .window
            .filter(|_| entry.phase == Phase::Settled)
            .ok_or_else(|| Error::Rejected("the session is still opening".into()))
    }

    pub(crate) fn load(&self, id: SessionId, url: String) {
        let tab = {
            let mut state = self.state();
            let Some(entry) = state.sessions.get_mut(&id) else {
                return;
            };
            match (entry.phase, entry.tab) {
                (Phase::Closing, _) => None,
                (_, Some(tab)) => Some(tab),
                (_, None) => {
                    entry.pending = Pending::Url(url.clone());
                    None
                }
            }
        };
        if let Some(tab) = tab {
            self.notify("tabs.update", vec![json!(tab), json!({ "url": url })]);
        }
    }

    pub(crate) fn unregister_scripts(&self, ids: &[&str]) {
        self.notify(
            "scripting.unregisterContentScripts",
            vec![json!({ "ids": ids })],
        );
    }

    pub(crate) fn request_report(&self, tab: TabId) {
        self.notify(
            "tabs.sendMessage",
            vec![
                json!(tab),
                json!({ "rikka": "report" }),
                json!({ "frameId": 0 }),
            ],
        );
    }

    pub(crate) fn abandon(&self, id: SessionId, error: &Error) {
        tracing::warn!(%error, "failed to open a Chrome tab");
        let Some(entry) = self.state().forget(id) else {
            return;
        };
        if let Some(tab) = entry.tab {
            self.notify("tabs.remove", vec![json!(tab)]);
        }
        if let Some(window) = entry.window {
            self.close_window(window);
        }
        entry.core.end(CloseReason::Page);
    }

    pub(crate) fn close(&self, id: SessionId) {
        let (core, window) = {
            let mut state = self.state();
            let Some(entry) = state.sessions.get_mut(&id) else {
                return;
            };
            let window = entry.window;
            match window {
                Some(_) => {
                    let entry = state.forget(id).expect("the entry exists");
                    (entry.core, window)
                }
                None => {
                    entry.phase = Phase::Closing;
                    (Arc::clone(&entry.core), None)
                }
            }
        };
        core.end(CloseReason::Requested);
        if let Some(window) = window {
            self.close_window(window);
        }
    }

    pub(crate) fn close_window(&self, window: WindowId) {
        self.notify("windows.remove", vec![json!(window)]);
    }

    pub(crate) fn refuse(&self, tab: TabId) {
        if self.state().orphans.remove(&tab).is_some() {
            self.notify("tabs.remove", vec![json!(tab)]);
        }
    }

    pub(crate) fn exited(&self, exit: Exit) {
        self.link.end();
        let (sessions, foreign): (Vec<_>, Vec<_>) = {
            let mut state = self.state();
            state.ended = true;
            state.tabs.clear();
            state.windows.clear();
            state.views.clear();
            state.orphans.clear();
            state.waiting.clear();
            state.unclaimed.clear();
            state.window_info.clear();
            state.page_scripts.clear();
            let sessions = state
                .sessions
                .drain()
                .map(|(_, entry)| entry.core)
                .collect();
            let foreign = state
                .foreign
                .drain()
                .flat_map(|(_, closed)| closed)
                .collect();
            (sessions, foreign)
        };
        for session in sessions {
            session.end(CloseReason::EngineExited);
        }
        for closed in foreign {
            closed();
        }
        self.outbox.send(RuntimeEvent::Exited(exit));
        self.outbox.close();
    }

    pub(crate) fn is_running(&self) -> bool {
        !self.state().ended
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.link.end();
        let Some(parting) = self.parting.lock().take() else {
            return;
        };
        let Parting {
            watch,
            listening,
            rendezvous,
            nested,
            display,
            writer,
        } = parting;
        watch.part(move || {
            threads::join(listening.stop());
            drop(rendezvous);
            drop(nested);
            if let Some(display) = display {
                threads::join(display);
            }
            drop(writer);
        });
    }
}
