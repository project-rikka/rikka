use rikka_seam::{Request, SeamClient};
use rikka_ui::{State, TextDocument, stream};
use shell_apps::Installed;
use shell_kit::{CLOSE, copy_to_clipboard};

use crate::CAPABILITY;
use crate::gallery::Turn;
use crate::provider::{Activation, Layout, ProviderInfo, Within};
use crate::registry::{Providers, Ranked};

#[derive(Debug, State)]
pub(super) struct Launcher {
    seam: SeamClient,
    providers: Providers,
    installed: Installed,
    query: TextDocument,
    triggered: Option<usize>,
    found: Vec<Ranked>,
    selected: usize,
    turn: Turn,
}

impl Launcher {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, providers: Providers, installed: Installed) -> Launcher {
        Launcher {
            seam,
            providers,
            installed,
            query: TextDocument::default(),
            triggered: None,
            found: Vec::new(),
            selected: 0,
            turn: Turn::default(),
        }
    }

    pub(crate) fn start(&mut self) {
        self.providers.reload();
        stream(self.installed.changes(), |launcher: &mut Launcher, ()| {
            launcher.refresh();
        });
        stream(self.providers.changes(), |launcher: &mut Launcher, ()| {
            launcher.refresh();
        });
        self.search();
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.query
    }

    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    pub(crate) fn turn(&self) -> Turn {
        self.turn
    }

    pub(super) fn triggered(&self) -> Option<usize> {
        self.triggered
    }

    pub(crate) fn within(&self) -> Option<Within> {
        self.providers.within(self.triggered?)
    }

    pub(crate) fn layout(&self) -> Layout {
        self.within()
            .map_or(Layout::default(), |within| within.layout())
    }

    pub(super) fn provider(&self, index: usize) -> Option<ProviderInfo> {
        self.providers.info(index)
    }

    pub(crate) fn shown(&self) -> impl Iterator<Item = &Ranked> {
        self.found.iter()
    }

    pub(crate) fn search(&mut self) {
        self.answer();
        self.selected = 0;
    }

    fn refresh(&mut self) {
        self.answer();
        self.selected = self.selected.min(self.found.len().saturating_sub(1));
    }

    fn answer(&mut self) {
        let reading = self.providers.read(&self.query.text());
        self.triggered = reading.triggered;
        self.found = self.providers.answer(&reading);
    }

    pub(crate) fn walk(&mut self, delta: i32) {
        let count = self.shown().count();
        if count == 0 {
            return;
        }
        let moved = self.selected as i32 + delta;
        self.selected = match self.layout() {
            Layout::Gallery => {
                self.turn = self.turn.then(i64::from(delta));
                moved.rem_euclid(count as i32)
            }
            Layout::List => moved.clamp(0, count as i32 - 1),
        } as usize;
    }

    pub(crate) fn select(&mut self, index: usize) {
        self.selected = index.min(self.shown().count().saturating_sub(1));
    }

    pub(crate) fn activate(&mut self, index: usize) {
        let Some(ranked) = self.shown().nth(index).cloned() else {
            return;
        };
        match self.providers.activate(&ranked) {
            Activation::Ask(request) => {
                self.seam.notify(*request);
                self.close();
            }
            Activation::Copy(text) => {
                copy_to_clipboard(&text);
                self.close();
            }
            Activation::Dismiss => self.close(),
            Activation::Complete(text) => self.retype(&text),
            Activation::Ignore => {}
        }
    }

    pub(super) fn complete(&mut self) {
        let completes = self
            .shown()
            .nth(self.selected)
            .is_some_and(|ranked| ranked.hit.completion().is_some());
        if completes {
            self.activate(self.selected);
        }
    }

    pub(crate) fn escape(&mut self) {
        match self.unwound() {
            Some(level) => self.retype(&level),
            None => self.close(),
        }
    }

    fn unwound(&self) -> Option<String> {
        let typed = self.query.text();
        let typed = typed.trim();
        if typed.is_empty() {
            return None;
        }
        let Some(info) = self.triggered.and_then(|index| self.provider(index)) else {
            return Some(String::new());
        };
        let typed_prefix = self.within().map(|within| info.typed(within.opened_by()));
        let level = [typed_prefix, Some(info.typed(""))]
            .into_iter()
            .flatten()
            .find(|level| !level.trim_end().eq_ignore_ascii_case(typed));
        Some(level.unwrap_or_default())
    }

    fn retype(&mut self, text: &str) {
        self.query.set_text(text);
        self.search();
    }

    fn close(&self) {
        self.seam.notify(Request::new(CAPABILITY, CLOSE));
    }
}
