mod builder;
pub(crate) mod chrome;
mod core;
pub(crate) mod dispatch;
mod display;
pub(crate) mod downloads;
pub(crate) mod joint;
mod sessions;
pub(crate) mod state;

use std::sync::Arc;

use futures_channel::mpsc::UnboundedReceiver;
use serde_json::Value;

pub use builder::RuntimeBuilder;
pub(crate) use core::Core;

use crate::api::{Account, Bookmarks, Devices, Downloads, Extensions, History, Privacy, Storage};
use crate::error::Result;
use crate::event::RuntimeEvent;
use crate::ids::{TabId, WindowId};
use crate::opening::Opening;
use crate::runtime::state::Pending;
use crate::script::{Frames, InjectAt, RELAY, RELAY_ID, UserScript, World};
use crate::session::{Session, SessionSettings};

#[derive(Clone, Debug)]
pub struct Runtime(Arc<Core>);

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::default()
    }

    pub(crate) fn from_core(core: Arc<Core>) -> Runtime {
        Runtime(core)
    }

    pub(crate) fn core(&self) -> &Arc<Core> {
        &self.0
    }

    #[must_use]
    pub fn events(&self) -> Option<UnboundedReceiver<RuntimeEvent>> {
        self.0.outbox.open()
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }

    #[must_use]
    pub fn extension_id(&self) -> &str {
        &self.0.extension_id
    }

    pub fn session(&self, settings: SessionSettings) -> Result<Session> {
        let url = settings.url.clone().unwrap_or_else(|| "about:blank".into());
        let core = self.0.open_session(&settings, Pending::Url(url), None)?;
        Ok(Session::new(core, self.clone()))
    }

    pub(crate) fn adopt(
        &self,
        tab: TabId,
        window: WindowId,
        opening: Opening,
        settings: SessionSettings,
    ) -> Result<Session> {
        let core = self.0.adopt_orphan(tab, window, opening, &settings)?;
        Ok(Session::new(core, self.clone()))
    }

    pub fn register(&self, script: &UserScript) {
        let (relay, withdrawn) = {
            let mut state = self.0.state();
            let was_empty = state.page_scripts.is_empty();
            match script.world {
                World::Page => {
                    state.page_scripts.insert(script.id.clone());
                    (was_empty, false)
                }
                World::Isolated => {
                    let removed = state.page_scripts.remove(&script.id);
                    (false, removed && state.page_scripts.is_empty())
                }
            }
        };
        let mut registrations = vec![script.registration()];
        if relay {
            let relay = UserScript::new(RELAY_ID, RELAY)
                .inject_at(InjectAt::Start)
                .frames(Frames::All);
            registrations.push(relay.registration());
        }
        self.0
            .notify("rikka.register", vec![Value::Array(registrations)]);
        if withdrawn {
            self.0.unregister_scripts(&[RELAY_ID]);
        }
    }

    pub fn unregister(&self, id: &str) {
        let withdrawn = {
            let mut state = self.0.state();
            state.page_scripts.remove(id) && state.page_scripts.is_empty()
        };
        match withdrawn {
            true => self.0.unregister_scripts(&[id, RELAY_ID]),
            false => self.0.unregister_scripts(&[id]),
        }
    }

    #[must_use]
    pub fn bundled_page(&self, path: &str) -> String {
        format!("chrome-extension://{}/scripts/{path}", self.0.extension_id)
    }

    #[must_use]
    pub fn history(&self) -> History {
        History::new(Arc::clone(&self.0.link))
    }

    #[must_use]
    pub fn bookmarks(&self) -> Bookmarks {
        Bookmarks::new(Arc::clone(&self.0.link))
    }

    #[must_use]
    pub fn downloads(&self) -> Downloads {
        Downloads::new(Arc::clone(&self.0.link))
    }

    #[must_use]
    pub fn storage(&self) -> Storage {
        Storage::new(Arc::clone(&self.0.link))
    }

    #[must_use]
    pub fn extensions(&self) -> Extensions {
        Extensions::new(Arc::clone(&self.0.link))
    }

    #[must_use]
    pub fn privacy(&self) -> Privacy {
        Privacy::new(Arc::clone(&self.0.link))
    }

    #[must_use]
    pub fn devices(&self) -> Devices {
        Devices::new(Arc::clone(&self.0.link))
    }

    pub fn account(&self) -> impl Future<Output = Result<Option<Account>>> + Send + use<> {
        Account::current(&self.0.link)
    }
}
