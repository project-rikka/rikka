use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rikka_nested::{ToplevelKey, ViewId};

use crate::foreign::ForeignKind;
use crate::ids::{SessionId, TabId, WindowId};
use crate::runtime::joint::JointHistory;
use crate::session::SessionCore;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    Url(String),
    Tab(TabId),
    Window,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Claiming,
    Pairing,
    Settled,
    Closing,
}

#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) core: Arc<SessionCore>,
    pub(crate) tab: Option<TabId>,
    pub(crate) window: Option<WindowId>,
    pub(crate) claim_tab: Option<TabId>,
    pub(crate) claim: String,
    pub(crate) attached: bool,
    pub(crate) pending: Pending,
    pub(crate) phase: Phase,
    pub(crate) fullscreen: bool,
    pub(crate) history: JointHistory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowKind {
    Normal,
    Popup,
    DevTools,
    Other,
}

impl WindowKind {
    pub(crate) fn parse(kind: Option<&str>) -> WindowKind {
        match kind {
            Some("normal") => WindowKind::Normal,
            Some("popup") => WindowKind::Popup,
            Some("devtools") => WindowKind::DevTools,
            _ => WindowKind::Other,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WindowInfo {
    pub(crate) kind: WindowKind,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct Orphan {
    pub(crate) url: String,
    pub(crate) title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
    Session(SessionId),
    Foreign(ForeignKind),
}

#[derive(Clone, Debug)]
pub(crate) struct Waiting {
    pub(crate) window: WindowId,
    pub(crate) purpose: Purpose,
    pub(crate) title: String,
}

#[derive(Clone, Debug)]
pub(crate) struct Unclaimed {
    pub(crate) toplevel: ToplevelKey,
    pub(crate) title: Option<String>,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) sessions: HashMap<SessionId, Entry>,
    pub(crate) tabs: HashMap<TabId, SessionId>,
    pub(crate) windows: HashMap<WindowId, SessionId>,
    pub(crate) views: HashMap<ViewId, SessionId>,
    pub(crate) window_info: HashMap<WindowId, WindowInfo>,
    pub(crate) orphans: HashMap<TabId, Orphan>,
    pub(crate) waiting: Vec<Waiting>,
    pub(crate) unclaimed: Vec<Unclaimed>,
    pub(crate) greeted: bool,
    pub(crate) ended: bool,
    pub(crate) foreign: HashMap<WindowId, Vec<Box<dyn FnOnce() + Send>>>,
    pub(crate) page_scripts: HashSet<String>,
}

impl State {
    pub(crate) fn session_of_tab(&self, tab: TabId) -> Option<&Entry> {
        self.sessions.get(self.tabs.get(&tab)?)
    }

    pub(crate) fn forget(&mut self, id: SessionId) -> Option<Entry> {
        let entry = self.sessions.remove(&id)?;
        if let Some(tab) = entry.tab {
            self.tabs.remove(&tab);
        }
        if let Some(window) = entry.window {
            self.windows.remove(&window);
        }
        self.views.remove(&entry.core.view.id());
        self.waiting
            .retain(|waiting| waiting.purpose != Purpose::Session(id));
        Some(entry)
    }
}
