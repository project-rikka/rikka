use std::collections::HashSet;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Weak};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::api::{Account, HistoryItem};
use crate::channel::{Hello, Inbound, Incoming, PROTOCOL};
use crate::context_menu::ContextReport;
use crate::error::Error;
use crate::event::{CloseReason, Event, Failure, RuntimeEvent};
use crate::foreign::ForeignKind;
use crate::ids::{SessionId, TabId, WindowId};
use crate::opening::{NewSessionRequest, Opening};
use crate::page::{PageState, Transition};
use crate::runtime::chrome::{
    ActiveInfo, ChromeWindow, Navigation, Origin, PageReport, Tab, ZoomChange,
};
use crate::runtime::core::Core;
use crate::runtime::joint::{Known, Step};
use crate::runtime::state::{Entry, Orphan, Phase, Purpose, Waiting, WindowInfo, WindowKind};
use crate::script::{ScriptAsk, ScriptMessage, World};
use crate::session::SessionCore;

pub(crate) struct Inlet(pub(crate) Weak<Core>);

impl Inbound for Inlet {
    fn connected(&self, stream: UnixStream) {
        if let Some(core) = self.0.upgrade() {
            core.link.connected(stream);
        }
    }

    fn received(&self, message: Incoming) {
        let Some(core) = self.0.upgrade() else {
            return;
        };
        match message {
            Incoming::Hello { hello } => core.greeted(&hello),
            Incoming::Reply {
                reply,
                value,
                error,
            } => core.link.replied(
                reply,
                match error {
                    Some(error) => Err(Error::Rejected(error)),
                    None => Ok(value),
                },
            ),
            Incoming::Event { event, args } => core.event(&event, args),
            Incoming::Ask { ask, kind, args } => core.asked(ask, &kind, args),
        }
    }

    fn disconnected(&self) {
        if let Some(core) = self.0.upgrade() {
            core.link.disconnected();
        }
    }

    fn overdue(&self) {
        if let Some(core) = self.0.upgrade() {
            core.failed(Failure::ExtensionMissing);
        }
    }
}

fn arg<T: DeserializeOwned>(args: &[Value], index: usize) -> Option<T> {
    let value = args.get(index)?.clone();
    match serde_json::from_value(value) {
        Ok(decoded) => Some(decoded),
        Err(error) => {
            tracing::warn!(%error, index, "malformed Chrome event argument");
            None
        }
    }
}

pub(crate) fn describe(page: &mut PageState, tab: &Tab) {
    if let Some(url) = tab.url.as_ref().filter(|url| !url.is_empty()) {
        page.url.clone_from(url);
    }
    if let Some(title) = &tab.title {
        page.title.clone_from(title);
    }
    page.loading = tab.status.as_deref() == Some("loading");
    page.favicon.clone_from(&tab.fav_icon_url);
    page.audible = tab.audible;
    page.muted = tab.muted_info.as_ref().is_some_and(|info| info.muted);
    page.discarded = tab.discarded;
}

impl Core {
    fn failed(&self, failure: Failure) {
        self.link.refuse();
        self.outbox.send(RuntimeEvent::Failed(failure));
    }

    fn greeted(&self, hello: &Hello) {
        if hello.protocol != PROTOCOL {
            self.failed(Failure::ExtensionOutdated {
                found: hello.protocol,
                expected: PROTOCOL,
            });
            return;
        }
        self.link.ready();
        let first = !std::mem::replace(&mut self.state().greeted, true);
        if first {
            self.outbox.send(RuntimeEvent::Ready {
                version: hello.version.clone(),
            });
        }
        self.downloads.refresh();
        let this = self.this.clone();
        self.link.call_then(
            "windows.getAll",
            vec![json!({ "populate": true, "windowTypes": ["normal", "popup", "devtools"] })],
            move |result| {
                let (Some(core), Ok(value)) = (this.upgrade(), result) else {
                    return;
                };
                match serde_json::from_value::<Vec<ChromeWindow>>(value) {
                    Ok(windows) => core.survey(windows, first),
                    Err(error) => tracing::warn!(%error, "malformed Chrome window list"),
                }
            },
        );
    }

    fn survey(&self, windows: Vec<ChromeWindow>, first: bool) {
        let mut stray = Vec::new();
        let gone: Vec<SessionId> = {
            let mut state = self.state();
            let present: HashSet<TabId> = windows
                .iter()
                .flat_map(|window| window.tabs.iter().filter_map(|tab| tab.id))
                .collect();
            for window in &windows {
                let Some(window_id) = window.id else {
                    continue;
                };
                state.window_info.insert(window_id, info(window));
                let ours = || {
                    window.tabs.iter().any(|tab| {
                        tab.id.is_some_and(|id| state.tabs.contains_key(&id))
                            || self.is_claim_url(&tab.address())
                    })
                };
                if first && !ours() {
                    stray.push(window_id);
                }
            }
            state
                .sessions
                .iter()
                .filter(|(_, entry)| {
                    entry.phase == Phase::Settled
                        && entry.tab.is_some_and(|tab| !present.contains(&tab))
                })
                .map(|(id, _)| *id)
                .collect()
        };
        for window in stray {
            self.close_window(window);
        }
        for id in gone {
            if let Some(entry) = self.state().forget(id) {
                entry.core.end(CloseReason::Page);
            }
        }
    }

    pub(crate) fn event(&self, name: &str, args: Vec<Value>) {
        match name {
            "tabs.onCreated" => {
                if let Some(tab) = arg::<Tab>(&args, 0) {
                    self.tab_created(&tab);
                }
            }
            "tabs.onUpdated" => {
                if let Some(tab) = arg::<Tab>(&args, 2) {
                    self.tab_updated(&tab);
                }
            }
            "tabs.onActivated" => {
                if let Some(active) = arg::<ActiveInfo>(&args, 0) {
                    self.tab_activated(&active);
                }
            }
            "tabs.onRemoved" => {
                if let Some(tab) = arg::<TabId>(&args, 0) {
                    self.tab_removed(tab);
                }
            }
            "tabs.onReplaced" => {
                if let (Some(added), Some(removed)) =
                    (arg::<TabId>(&args, 0), arg::<TabId>(&args, 1))
                {
                    self.tab_replaced(added, removed);
                }
            }
            "tabs.onZoomChange" => {
                if let Some(change) = arg::<ZoomChange>(&args, 0) {
                    self.with_tab(change.tab_id, |session| {
                        session.update(|page| page.zoom = change.new_zoom_factor);
                    });
                }
            }
            "windows.onCreated" => {
                if let Some(window) = arg::<ChromeWindow>(&args, 0) {
                    self.window_created(&window);
                }
            }
            "windows.onRemoved" => {
                if let Some(window) = arg::<WindowId>(&args, 0) {
                    self.window_removed(window);
                }
            }
            "webNavigation.onBeforeNavigate"
            | "webNavigation.onCommitted"
            | "webNavigation.onHistoryStateUpdated"
            | "webNavigation.onReferenceFragmentUpdated"
            | "webNavigation.onCompleted"
            | "webNavigation.onErrorOccurred" => {
                if let Some(navigation) = arg::<Navigation>(&args, 0) {
                    self.navigated(name, navigation);
                }
            }
            "rikka.page" => {
                if let (Some(origin), Some(report)) =
                    (arg::<Origin>(&args, 0), arg::<PageReport>(&args, 1))
                {
                    self.page_reported(&origin, &report);
                }
            }
            "rikka.post" => {
                if let (Some(origin), Some(name), Some(world)) = (
                    arg::<Origin>(&args, 0),
                    arg::<String>(&args, 1),
                    arg::<String>(&args, 3),
                ) {
                    let message = ScriptMessage {
                        name,
                        body: args.get(2).cloned().unwrap_or(Value::Null),
                        world: World::parse(&world),
                        frame: origin.frame,
                        url: origin.url.clone(),
                    };
                    if let Some(tab) = origin.tab {
                        self.with_tab(tab, |session| session.send(Event::Message(message)));
                    }
                }
            }
            "rikka.context" => {
                if let (Some(origin), Some(report)) =
                    (arg::<Origin>(&args, 0), arg::<ContextReport>(&args, 1))
                    && let Some(tab) = origin.tab
                {
                    self.with_tab(tab, |session| {
                        let menu = report.menu(origin.frame, origin.url, session.take_press());
                        session.send(Event::ContextMenu(menu));
                    });
                }
            }
            "rikka.hover" => {
                if let (Some(origin), Some(url)) =
                    (arg::<Origin>(&args, 0), arg::<Option<String>>(&args, 1))
                    && let Some(tab) = origin.tab
                {
                    self.with_tab(tab, |session| session.hover(origin.frame, url));
                }
            }
            "downloads.onCreated" | "downloads.onChanged" | "downloads.onErased" => {
                self.downloads.refresh();
            }
            "history.onVisited" => {
                if let Some(item) = arg::<HistoryItem>(&args, 0)
                    && !self.is_claim_url(&item.url)
                {
                    self.outbox.send(RuntimeEvent::Visited(item));
                }
            }
            "history.onVisitRemoved" => {
                #[derive(serde::Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Removed {
                    all_history: bool,
                    #[serde(default)]
                    urls: Vec<String>,
                }
                if let Some(removed) = arg::<Removed>(&args, 0) {
                    self.outbox.send(RuntimeEvent::VisitsRemoved {
                        all: removed.all_history,
                        urls: removed.urls,
                    });
                }
            }
            "bookmarks.onChanged"
            | "bookmarks.onChildrenReordered"
            | "bookmarks.onCreated"
            | "bookmarks.onMoved"
            | "bookmarks.onRemoved" => self.outbox.send(RuntimeEvent::BookmarksChanged),
            "management.onDisabled"
            | "management.onEnabled"
            | "management.onInstalled"
            | "management.onUninstalled" => self.outbox.send(RuntimeEvent::ExtensionsChanged),
            "identity.onSignInChanged" => self.account_changed(),
            _ => {}
        }
    }

    fn asked(&self, ask: u64, kind: &str, args: Vec<Value>) {
        let message = (kind == "message")
            .then(|| (arg::<Origin>(&args, 0), arg::<String>(&args, 1)))
            .and_then(|(origin, name)| Some((origin?, name?)));
        let Some((origin, name)) = message else {
            self.link.answer(ask, &Value::Null);
            return;
        };
        let session = origin.tab.and_then(|tab| {
            self.state()
                .session_of_tab(tab)
                .map(|entry| Arc::clone(&entry.core))
        });
        let Some(session) = session else {
            self.link.answer(ask, &Value::Null);
            return;
        };
        let message = ScriptMessage {
            name,
            body: args.get(2).cloned().unwrap_or(Value::Null),
            world: World::Isolated,
            frame: origin.frame,
            url: origin.url,
        };
        session.send(Event::Ask(ScriptAsk::new(
            message,
            ask,
            Arc::clone(&self.link),
        )));
    }

    fn with_tab(&self, tab: TabId, action: impl FnOnce(&SessionCore)) {
        let session = self
            .state()
            .session_of_tab(tab)
            .map(|entry| Arc::clone(&entry.core));
        if let Some(session) = session {
            action(&session);
        }
    }

    fn tab_created(&self, tab: &Tab) {
        let Some(tab_id) = tab.id else {
            return;
        };
        if self.is_claim_url(&tab.address()) {
            return;
        }
        let settling = {
            let state = self.state();
            if state.tabs.contains_key(&tab_id) {
                return;
            }
            state.windows.get(&tab.window_id).copied().filter(|id| {
                state.sessions.get(id).is_some_and(|entry| {
                    entry.tab.is_none()
                        && matches!(entry.pending, crate::runtime::state::Pending::Url(_))
                })
            })
        };
        if let Some(id) = settling {
            self.own_tab(id, tab);
            return;
        }
        let (opener, opener_window, opener_tab, info) = {
            let state = self.state();
            let opener = tab
                .opener_tab_id
                .and_then(|opener| state.tabs.get(&opener).copied());
            let entry = opener.and_then(|id| state.sessions.get(&id));
            (
                opener,
                entry.and_then(|entry| entry.window),
                entry.and_then(|entry| entry.tab),
                state.window_info.get(&tab.window_id).copied(),
            )
        };
        let in_opener_window = opener_window == Some(tab.window_id);
        let opening = match (in_opener_window, info.map(|info| info.kind)) {
            (true, _) if tab.active => Opening::ForegroundTab,
            (true, _) => Opening::BackgroundTab,
            (false, Some(WindowKind::Popup)) => Opening::Popup {
                width: info.and_then(|info| info.width),
                height: info.and_then(|info| info.height),
            },
            (false, _) => Opening::Window,
        };
        self.state().orphans.insert(tab_id, orphan(tab));
        let request = NewSessionRequest::new(
            self.this.clone(),
            tab_id,
            tab.window_id,
            opening,
            tab.address(),
            tab.title.clone().unwrap_or_default(),
        );
        if in_opener_window
            && tab.active
            && let Some(opener_tab) = opener_tab
        {
            self.notify(
                "tabs.update",
                vec![json!(opener_tab), json!({ "active": true })],
            );
        }
        let session = opener.and_then(|id| {
            self.state()
                .sessions
                .get(&id)
                .map(|entry| Arc::clone(&entry.core))
        });
        match session {
            Some(session) => session.send(Event::NewSession(request)),
            None => self.outbox.send(RuntimeEvent::NewSession(request)),
        }
    }

    fn tab_updated(&self, tab: &Tab) {
        let Some(tab_id) = tab.id else {
            return;
        };
        let (session, waiting_title) = {
            let mut state = self.state();
            if let Some(orphan) = state.orphans.get_mut(&tab_id) {
                orphan.url = tab.address();
                orphan.title = tab.title.clone().unwrap_or_default();
            }
            let session = state
                .session_of_tab(tab_id)
                .map(|entry| (entry.core.id, Arc::clone(&entry.core)));
            let waiting_title = session.as_ref().and_then(|(id, _)| {
                let title = tab.title.clone().filter(|title| !title.is_empty())?;
                let waiting = state
                    .waiting
                    .iter_mut()
                    .find(|waiting| waiting.purpose == Purpose::Session(*id))?;
                waiting.title = title;
                Some(())
            });
            (session.map(|(_, core)| core), waiting_title)
        };
        if let Some(session) = session {
            session.update(|page| describe(page, tab));
        }
        if waiting_title.is_some() {
            self.pair();
        }
    }

    fn tab_activated(&self, active: &ActiveInfo) {
        let restore = {
            let state = self.state();
            state
                .windows
                .get(&active.window_id)
                .and_then(|id| state.sessions.get(id))
                .filter(|entry| entry.phase == Phase::Settled)
                .and_then(|entry| entry.tab)
                .filter(|tab| *tab != active.tab_id && !state.tabs.contains_key(&active.tab_id))
        };
        if let Some(tab) = restore {
            self.notify("tabs.update", vec![json!(tab), json!({ "active": true })]);
        }
    }

    fn tab_removed(&self, tab: TabId) {
        let ended = {
            let mut state = self.state();
            state.orphans.remove(&tab);
            let id = state.tabs.get(&tab).copied();
            id.and_then(|id| {
                let moving = state
                    .sessions
                    .get(&id)
                    .is_some_and(|entry| entry.tab != Some(tab));
                (!moving).then(|| state.forget(id)).flatten()
            })
        };
        if let Some(entry) = ended {
            if let Some(window) = entry.window {
                self.close_window(window);
            }
            entry.core.end(CloseReason::Page);
        }
    }

    fn tab_replaced(&self, added: TabId, removed: TabId) {
        let mut state = self.state();
        let Some(id) = state.tabs.remove(&removed) else {
            return;
        };
        state.tabs.insert(added, id);
        if let Some(entry) = state.sessions.get_mut(&id) {
            entry.tab = Some(added);
        }
    }

    fn window_created(&self, window: &ChromeWindow) {
        let Some(window_id) = window.id else {
            return;
        };
        let info = info(window);
        {
            let mut state = self.state();
            state.window_info.insert(window_id, info);
            if info.kind == WindowKind::DevTools {
                state.waiting.push(Waiting {
                    window: window_id,
                    purpose: Purpose::Foreign(ForeignKind::DevTools),
                    title: "DevTools".into(),
                });
            }
        }
        if info.kind == WindowKind::DevTools {
            self.pair();
        }
    }

    fn navigated(&self, name: &str, navigation: Navigation) {
        if navigation.frame_id != 0 {
            return;
        }
        let committed = matches!(
            name,
            "webNavigation.onCommitted"
                | "webNavigation.onHistoryStateUpdated"
                | "webNavigation.onReferenceFragmentUpdated"
        );
        if committed {
            let step = step_of(&navigation);
            self.walk(navigation.tab_id, |entry| {
                entry.history.committed(&navigation.url, step);
            });
        }
        let url = navigation.url;
        let event = match name {
            "webNavigation.onBeforeNavigate" => Event::Started { url },
            "webNavigation.onReferenceFragmentUpdated" => return,
            "webNavigation.onCommitted" | "webNavigation.onHistoryStateUpdated" => {
                if name == "webNavigation.onCommitted" {
                    self.with_tab(navigation.tab_id, SessionCore::clear_hover);
                }
                Event::Committed {
                    transition: Transition::parse(
                        navigation.transition_type.as_deref().unwrap_or("link"),
                        &navigation.transition_qualifiers,
                    ),
                    url,
                }
            }
            "webNavigation.onCompleted" => Event::Finished { url },
            _ => Event::Failed {
                url,
                error: navigation.error.unwrap_or_default(),
            },
        };
        self.with_tab(navigation.tab_id, |session| session.send(event));
    }

    fn page_reported(&self, origin: &Origin, report: &PageReport) {
        let Some(tab) = origin.tab else {
            return;
        };
        let mut toggled = false;
        let walked = self.walk(tab, |entry| {
            toggled = entry.fullscreen != report.fullscreen;
            entry.fullscreen = report.fullscreen;
            entry.history.reported(
                &report.url,
                report.step,
                report.length,
                Known {
                    back: report.back,
                    forward: report.forward,
                },
            );
        });
        let Some(session) = walked else {
            return;
        };
        session.update(|page| page.fullscreen = report.fullscreen);
        if toggled {
            session.send(Event::Fullscreen(report.fullscreen));
        }
    }

    fn walk(&self, tab: TabId, change: impl FnOnce(&mut Entry)) -> Option<Arc<SessionCore>> {
        let (session, back, forward) = {
            let mut state = self.state();
            let id = state.tabs.get(&tab).copied()?;
            let entry = state.sessions.get_mut(&id)?;
            change(entry);
            (
                Arc::clone(&entry.core),
                entry.history.can_go_back(),
                entry.history.can_go_forward(),
            )
        };
        session.update(|page| {
            page.can_go_back = back;
            page.can_go_forward = forward;
        });
        Some(session)
    }

    fn window_removed(&self, window: WindowId) {
        let (closed, ended) = {
            let mut state = self.state();
            state.window_info.remove(&window);
            state.waiting.retain(|waiting| waiting.window != window);
            let ended = state
                .windows
                .get(&window)
                .copied()
                .and_then(|id| state.forget(id));
            (state.foreign.remove(&window).unwrap_or_default(), ended)
        };
        for watcher in closed {
            watcher();
        }
        if let Some(entry) = ended {
            entry.core.end(CloseReason::Page);
        }
    }

    fn account_changed(&self) {
        let this = self.this.clone();
        Account::current_then(&self.link, move |account| {
            let Some(core) = this.upgrade() else {
                return;
            };
            match account {
                Ok(account) => core.outbox.send(RuntimeEvent::AccountChanged(account)),
                Err(error) => tracing::warn!(%error, "failed to read the Google account"),
            }
        });
    }

    pub(crate) fn activation_requested(&self, id: SessionId) {
        let session = self
            .state()
            .sessions
            .get(&id)
            .map(|entry| Arc::clone(&entry.core));
        if let Some(session) = session {
            session.send(Event::ActivationRequested);
        }
    }
}

fn info(window: &ChromeWindow) -> WindowInfo {
    WindowInfo {
        kind: WindowKind::parse(window.kind.as_deref()),
        width: window.width,
        height: window.height,
    }
}

fn step_of(navigation: &Navigation) -> Step {
    let traversed = navigation
        .transition_qualifiers
        .iter()
        .any(|qualifier| qualifier == "forward_back");
    match navigation.transition_type.as_deref() {
        _ if traversed => Step::Traverse,
        Some("reload") => Step::Reload,
        _ => Step::Push,
    }
}

fn orphan(tab: &Tab) -> Orphan {
    Orphan {
        url: tab.address(),
        title: tab.title.clone().unwrap_or_default(),
    }
}
