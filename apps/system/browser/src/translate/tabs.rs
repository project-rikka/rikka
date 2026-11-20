use std::collections::{HashMap, VecDeque};

use browser_translators::{CodeMode, Command, Display, Engine, Watch};
use rikka_languages::Language;
use rikka_ui::Revision;
use rikka_web::ScriptReply;

use crate::translate::failure::TranslateFailure;
use browser_model::TabId;

const MAX_WATCHERS: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Progress {
    #[default]
    Original,
    Translating,
    Translated,
    Failed(TranslateFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Session {
    pub(crate) id: u64,
    pub(crate) engine: Engine,
    pub(crate) to: Language,
    pub(crate) display: Display,
    pub(crate) code: CodeMode,
}

impl Session {
    pub(crate) fn command(&self) -> Command {
        Command::Translate {
            session: self.id,
            to: self.to,
            display: self.display,
            code: self.code,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Follow {
    pub(crate) source: Option<Language>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Choice {
    pub(crate) engine: Engine,
    pub(crate) display: Display,
    pub(crate) code: CodeMode,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TabStatus {
    pub(crate) detected: Option<Language>,
    pub(crate) progress: Progress,
    pub(crate) busy: bool,
}

#[derive(Debug)]
pub(crate) struct PendingCommand {
    watchers: Vec<Watcher>,
    command: Command,
}

impl PendingCommand {
    pub(crate) fn send(self) {
        for watcher in self.watchers {
            watcher.reply.send(self.command.answer(&watcher.frame));
        }
    }
}

#[derive(Debug)]
struct Watcher {
    frame: Watch,
    reply: ScriptReply,
}

#[derive(Debug)]
struct TabTranslation {
    view: rikka_web::Session,
    detected: Option<Language>,
    progress: Progress,
    session: Option<Session>,
    watchers: VecDeque<Watcher>,
    in_flight: usize,
    follow: Option<Follow>,
}

impl TabTranslation {
    fn new(view: rikka_web::Session) -> TabTranslation {
        TabTranslation {
            view,
            detected: None,
            progress: Progress::Original,
            session: None,
            watchers: VecDeque::new(),
            in_flight: 0,
            follow: None,
        }
    }

    fn take_command(&mut self) -> PendingCommand {
        let command = match &self.session {
            Some(session) => session.command(),
            None => Command::Restore,
        };
        PendingCommand {
            watchers: self.watchers.drain(..).collect(),
            command,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Tabs {
    tabs: HashMap<TabId, TabTranslation>,
    sessions: u64,
    revision: Revision,
}

impl Tabs {
    pub(crate) fn status(&self, tab: TabId) -> TabStatus {
        self.revision.read();
        let Some(state) = self.tabs.get(&tab) else {
            return TabStatus::default();
        };
        TabStatus {
            detected: state.detected,
            progress: state.progress.clone(),
            busy: state.in_flight > 0,
        }
    }

    pub(crate) fn shows_translation(&self, tab: TabId) -> bool {
        self.revision.read();
        self.tabs
            .get(&tab)
            .is_some_and(|state| state.session.is_some())
    }

    pub(crate) fn session(&self, tab: TabId) -> Option<&Session> {
        self.tabs.get(&tab)?.session.as_ref()
    }

    pub(crate) fn follow(&self, tab: TabId) -> Option<&Follow> {
        self.tabs.get(&tab)?.follow.as_ref()
    }

    pub(crate) fn attach(&mut self, tab: TabId, view: &rikka_web::Session) {
        if self.tabs.get(&tab).is_some_and(|state| state.view == *view) {
            return;
        }
        self.tabs.insert(tab, TabTranslation::new(view.clone()));
        self.revision.changed();
    }

    pub(crate) fn detected(&mut self, tab: TabId, language: Option<Language>) {
        let Some(state) = self.tabs.get_mut(&tab) else {
            return;
        };
        if state.detected != language {
            state.detected = language;
            self.revision.changed();
        }
    }

    pub(crate) fn hold(
        &mut self,
        tab: TabId,
        frame: Watch,
        reply: ScriptReply,
    ) -> Option<ScriptReply> {
        let Some(state) = self.tabs.get_mut(&tab) else {
            return Some(reply);
        };
        state.watchers.push_back(Watcher { frame, reply });
        if state.watchers.len() <= MAX_WATCHERS {
            return None;
        }
        let oldest = state
            .watchers
            .iter()
            .position(|watcher| !watcher.frame.top)?;
        state.watchers.remove(oldest).map(|watcher| watcher.reply)
    }

    pub(crate) fn start(
        &mut self,
        tab: TabId,
        to: Language,
        choice: Choice,
    ) -> Option<PendingCommand> {
        let state = self.tabs.get_mut(&tab)?;
        self.sessions += 1;
        state.follow = Some(Follow {
            source: state.detected,
        });
        state.session = Some(Session {
            id: self.sessions,
            engine: choice.engine,
            to,
            display: choice.display,
            code: choice.code,
        });
        state.progress = Progress::Translating;
        state.in_flight = 0;
        self.revision.changed();
        Some(state.take_command())
    }

    pub(crate) fn restore(&mut self, tab: TabId) -> Option<PendingCommand> {
        let state = self.tabs.get_mut(&tab)?;
        state.follow = None;
        if state.session.is_none() && state.progress == Progress::Original {
            return None;
        }
        state.session = None;
        state.progress = Progress::Original;
        state.in_flight = 0;
        self.revision.changed();
        Some(state.take_command())
    }

    pub(crate) fn redisplay(&mut self, tab: TabId, display: Display) -> Option<PendingCommand> {
        let state = self.tabs.get_mut(&tab)?;
        let session = state.session.as_mut()?;
        if session.display == display {
            return None;
        }
        session.display = display;
        Some(state.take_command())
    }

    pub(crate) fn current(&self, tab: TabId, session: u64) -> Option<Session> {
        self.session(tab)
            .filter(|current| current.id == session)
            .cloned()
    }

    pub(crate) fn sent(&mut self, tab: TabId, session: u64) {
        let Some(state) = self.live(tab, session) else {
            return;
        };
        state.in_flight += 1;
        if state.in_flight == 1 {
            self.revision.changed();
        }
    }

    pub(crate) fn batch_finished(
        &mut self,
        tab: TabId,
        session: u64,
        outcome: Result<Option<Language>, TranslateFailure>,
    ) {
        let Some(state) = self.live(tab, session) else {
            return;
        };
        let before = (state.progress.clone(), state.detected);
        state.in_flight = state.in_flight.saturating_sub(1);
        match outcome {
            Ok(detected) => {
                state.progress = Progress::Translated;
                if let Some(detected) = detected {
                    if let Some(follow) = state
                        .follow
                        .as_mut()
                        .filter(|follow| follow.source.is_none())
                    {
                        follow.source = Some(detected);
                    }
                    state.detected = Some(detected);
                }
            }
            Err(failure) if state.progress == Progress::Translating => {
                state.progress = Progress::Failed(failure);
            }
            Err(_) => {}
        }
        if state.in_flight == 0 || before != (state.progress.clone(), state.detected) {
            self.revision.changed();
        }
    }

    pub(crate) fn failed(&mut self, tab: TabId, session: u64, failure: TranslateFailure) {
        let Some(state) = self.live(tab, session) else {
            return;
        };
        state.progress = Progress::Failed(failure);
        self.revision.changed();
    }

    pub(crate) fn navigated(&mut self, tab: TabId) {
        let Some(state) = self.tabs.get_mut(&tab) else {
            return;
        };
        let fresh = TabTranslation {
            follow: state.follow.take(),
            ..TabTranslation::new(state.view.clone())
        };
        *state = fresh;
        self.revision.changed();
    }

    pub(crate) fn retain(&mut self, alive: impl Fn(TabId, &rikka_web::Session) -> bool) {
        let before = self.tabs.len();
        self.tabs.retain(|tab, state| alive(*tab, &state.view));
        if self.tabs.len() != before {
            self.revision.changed();
        }
    }

    fn live(&mut self, tab: TabId, session: u64) -> Option<&mut TabTranslation> {
        self.tabs.get_mut(&tab).filter(|state| {
            state
                .session
                .as_ref()
                .is_some_and(|current| current.id == session)
        })
    }
}
