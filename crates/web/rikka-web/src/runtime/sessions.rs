use std::sync::Arc;

use rikka_nested::{Claim, ViewId};
use serde_json::{Value, json};

use crate::channel::decode;
use crate::error::{Error, Result};
use crate::event::RuntimeEvent;
use crate::foreign::ForeignWindow;
use crate::ids::{SessionId, TabId, WindowId};
use crate::opening::Opening;
use crate::runtime::chrome::{ChromeWindow, Tab};
use crate::runtime::core::{CLAIM_PREFIX, Core};
use crate::runtime::joint::JointHistory;
use crate::runtime::state::{Entry, Pending, Phase, Purpose, Unclaimed, Waiting};
use crate::session::{DEFAULT_SIZE, SessionCore, SessionSettings};

const TOKEN_BYTES: usize = 16;

fn token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).expect("the kernel provides randomness");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl Core {
    pub(crate) fn open_session(
        &self,
        settings: &SessionSettings,
        pending: Pending,
        page: Option<&Tab>,
    ) -> Result<Arc<SessionCore>> {
        let token = token();
        let view = self
            .nested
            .view(Claim::title(format!("{CLAIM_PREFIX}{token}")));
        view.set_fullscreen(true);
        view.resize(settings.size.unwrap_or(DEFAULT_SIZE), 1.0);
        let id = SessionId::next();
        let url = match &pending {
            Pending::Url(url) => url.clone(),
            Pending::Tab(_) | Pending::Window => page.map(Tab::address).unwrap_or_default(),
        };
        let core = Arc::new(SessionCore::new(id, view, url));
        if let Some(tab) = page {
            core.update(|state| {
                state.title = tab.title.clone().unwrap_or_default();
            });
        }
        let phase = match pending {
            Pending::Window => Phase::Pairing,
            Pending::Url(_) | Pending::Tab(_) => Phase::Claiming,
        };
        let tab = match &pending {
            Pending::Tab(tab) => Some(*tab),
            Pending::Window => page.and_then(|page| page.id),
            Pending::Url(_) => None,
        };
        let window = match pending {
            Pending::Window => page.map(|page| page.window_id),
            Pending::Url(_) | Pending::Tab(_) => None,
        };
        let entry = Entry {
            core: Arc::clone(&core),
            tab,
            window,
            claim_tab: None,
            claim: self.claim_url(&token),
            attached: false,
            pending,
            phase,
            fullscreen: false,
            history: JointHistory::default(),
        };
        {
            let mut state = self.state();
            if state.ended {
                return Err(Error::EngineExited);
            }
            state.views.insert(core.view.id(), id);
            if let Some(tab) = tab {
                state.tabs.insert(tab, id);
            }
            if let Some(window) = window {
                state.windows.insert(window, id);
            }
            state.sessions.insert(id, entry);
        }
        if phase == Phase::Claiming {
            let this = self.this.clone();
            self.link.call_then(
                "windows.create",
                vec![json!({ "url": self.claim_url(&token), "focused": false, "type": "normal" })],
                move |result| {
                    if let Some(core) = this.upgrade() {
                        core.claim_created(id, result);
                    }
                },
            );
        }
        Ok(core)
    }

    pub(crate) fn adopt_orphan(
        &self,
        tab: TabId,
        window: WindowId,
        opening: Opening,
        settings: &SessionSettings,
    ) -> Result<Arc<SessionCore>> {
        let orphan = self.state().orphans.remove(&tab).ok_or(Error::Closed)?;
        let page = Tab {
            id: Some(tab),
            window_id: window,
            opener_tab_id: None,
            url: Some(orphan.url),
            pending_url: None,
            title: Some(orphan.title.clone()),
            status: None,
            fav_icon_url: None,
            active: true,
            audible: false,
            muted_info: None,
            discarded: false,
        };
        let core = match opening {
            Opening::Popup { .. } => {
                let core = self.open_session(settings, Pending::Window, Some(&page))?;
                self.state().waiting.push(Waiting {
                    window,
                    purpose: Purpose::Session(core.id),
                    title: orphan.title,
                });
                self.pair();
                core
            }
            Opening::ForegroundTab | Opening::BackgroundTab | Opening::Window => {
                self.open_session(settings, Pending::Tab(tab), Some(&page))?
            }
        };
        self.request_report(tab);
        Ok(core)
    }

    fn claim_created(&self, id: SessionId, result: Result<Value>) {
        let window = match decode::<ChromeWindow>(result) {
            Ok(window) => window,
            Err(error) => {
                self.abandon(id, &error);
                return;
            }
        };
        let Some(window_id) = window.id else {
            self.abandon(id, &Error::Malformed("a window without an id".into()));
            return;
        };
        let claim_tab = window.tabs.first().and_then(|tab| tab.id);
        let closing = {
            let mut state = self.state();
            match state.sessions.get_mut(&id) {
                Some(entry) if entry.phase == Phase::Closing => {
                    state.forget(id);
                    true
                }
                Some(entry) => {
                    entry.window = Some(window_id);
                    entry.claim_tab = claim_tab;
                    state.windows.insert(window_id, id);
                    false
                }
                None => true,
            }
        };
        if closing {
            self.close_window(window_id);
            return;
        }
        self.settle(id);
    }

    pub(crate) fn attached(&self, view: ViewId) {
        let id = {
            let mut state = self.state();
            let Some(id) = state.views.get(&view).copied() else {
                return;
            };
            if let Some(entry) = state.sessions.get_mut(&id) {
                entry.attached = true;
            }
            id
        };
        self.settle(id);
    }

    fn settle(&self, id: SessionId) {
        let (window, claim_tab, claim, pending) = {
            let mut state = self.state();
            let Some(entry) = state.sessions.get_mut(&id) else {
                return;
            };
            let (Phase::Claiming, true, Some(window), Some(claim_tab)) =
                (entry.phase, entry.attached, entry.window, entry.claim_tab)
            else {
                return;
            };
            entry.phase = Phase::Settled;
            (
                window,
                claim_tab,
                entry.claim.clone(),
                entry.pending.clone(),
            )
        };
        let this = self.this.clone();
        let settled = move |owned: Result<()>| {
            let Some(core) = this.upgrade() else {
                return;
            };
            match owned {
                Ok(()) => {
                    core.notify("tabs.remove", vec![json!(claim_tab)]);
                    core.notify("history.deleteUrl", vec![json!({ "url": claim })]);
                }
                Err(error) => core.abandon(id, &error),
            }
        };
        match pending {
            Pending::Url(url) => {
                let this = self.this.clone();
                self.link.call_then(
                    "tabs.create",
                    vec![json!({ "windowId": window, "url": url, "active": true })],
                    move |result| {
                        let owned = decode::<Tab>(result).and_then(|tab| {
                            let core = this.upgrade().ok_or(Error::EngineExited)?;
                            tab.id
                                .ok_or_else(|| Error::Malformed("a tab without an id".into()))?;
                            core.own_tab(id, &tab);
                            Ok(())
                        });
                        settled(owned);
                    },
                );
            }
            Pending::Tab(tab) => {
                let this = self.this.clone();
                self.link.call_then(
                    "tabs.move",
                    vec![json!(tab), json!({ "windowId": window, "index": -1 })],
                    move |result| {
                        if result.is_ok()
                            && let Some(core) = this.upgrade()
                        {
                            core.notify("tabs.update", vec![json!(tab), json!({ "active": true })]);
                        }
                        settled(result.map(|_| ()));
                    },
                );
            }
            Pending::Window => {}
        }
    }

    pub(crate) fn own_tab(&self, id: SessionId, tab: &Tab) {
        let Some(tab_id) = tab.id else {
            return;
        };
        let core = {
            let mut state = self.state();
            let Some(entry) = state.sessions.get_mut(&id) else {
                return;
            };
            if entry.tab.is_some() {
                return;
            }
            entry.tab = Some(tab_id);
            let core = Arc::clone(&entry.core);
            state.tabs.insert(tab_id, id);
            core
        };
        core.update(|page| crate::runtime::dispatch::describe(page, tab));
    }

    pub(crate) fn unclaimed(&self, toplevel: rikka_nested::ToplevelKey, title: Option<String>) {
        {
            let mut state = self.state();
            match state
                .unclaimed
                .iter_mut()
                .find(|known| known.toplevel == toplevel)
            {
                Some(known) => known.title = title,
                None => state.unclaimed.push(Unclaimed { toplevel, title }),
            }
        }
        self.pair();
    }

    pub(crate) fn claimed(&self, toplevel: rikka_nested::ToplevelKey) {
        self.state()
            .unclaimed
            .retain(|known| known.toplevel != toplevel);
    }

    pub(crate) fn pair(&self) {
        loop {
            let paired = {
                let mut state = self.state();
                let candidates: Vec<usize> = state
                    .unclaimed
                    .iter()
                    .enumerate()
                    .filter(|(_, known)| {
                        !known
                            .title
                            .as_deref()
                            .is_some_and(|title| title.starts_with(CLAIM_PREFIX))
                    })
                    .map(|(index, _)| index)
                    .collect();
                let matched = state.waiting.iter().enumerate().find_map(|(at, waiting)| {
                    candidates
                        .iter()
                        .find(|index| {
                            !waiting.title.is_empty()
                                && state.unclaimed[**index]
                                    .title
                                    .as_deref()
                                    .is_some_and(|title| title.starts_with(&waiting.title))
                        })
                        .map(|index| (at, *index))
                });
                matched.map(|(at, index)| {
                    let waiting = state.waiting.remove(at);
                    let unclaimed = state.unclaimed.remove(index);
                    (waiting, unclaimed)
                })
            };
            let Some((waiting, unclaimed)) = paired else {
                return;
            };
            self.bind(&waiting, unclaimed);
        }
    }

    fn bind(&self, waiting: &Waiting, unclaimed: Unclaimed) {
        let claim = Claim::toplevel(unclaimed.toplevel);
        match waiting.purpose {
            Purpose::Session(id) => {
                let view = {
                    let mut state = self.state();
                    let Some(entry) = state.sessions.get_mut(&id) else {
                        return;
                    };
                    entry.phase = Phase::Settled;
                    entry.core.view.clone()
                };
                view.reclaim(claim);
            }
            Purpose::Foreign(kind) => {
                self.state().foreign.insert(waiting.window, Vec::new());
                let view = self.nested.view(claim);
                view.resize(DEFAULT_SIZE, 1.0);
                self.outbox.send(RuntimeEvent::ForeignWindow(ForeignWindow {
                    kind,
                    view,
                    window: waiting.window,
                    core: self.this.clone(),
                }));
            }
        }
    }
}
