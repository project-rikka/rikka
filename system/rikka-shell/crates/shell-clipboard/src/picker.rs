use rikka_seam::{Request, SeamClient};
use rikka_ui::{Label, State, TextDocument, tr};
use shell_kit::AimTarget;

use crate::entry::Entry;
use crate::history::History;
use crate::service::{CAPABILITY, CLOSE, COPY, PASTE};

const HISTORY_FULL: &str = "shell.clipboard.full";

#[derive(Debug, State)]
pub(crate) struct Picker {
    seam: SeamClient,
    history: History,
    entries: Vec<Entry>,
    shown: Vec<usize>,
    query: TextDocument,
    selected: usize,
    aim: Option<AimTarget>,
}

impl Picker {
    #[must_use]
    pub(crate) fn new(seam: SeamClient, history: History, aim: Option<AimTarget>) -> Picker {
        Picker {
            seam,
            history,
            entries: Vec::new(),
            shown: Vec::new(),
            query: TextDocument::default(),
            selected: 0,
            aim,
        }
    }

    pub(crate) fn field(&self) -> &TextDocument {
        &self.query
    }

    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    pub(crate) fn aim(&self) -> Option<AimTarget> {
        self.aim
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn has_pinned(&self) -> bool {
        self.entries.iter().any(|entry| entry.pinned)
    }

    pub(crate) fn trouble(&self) -> Option<Label> {
        self.history.turned_away().then(|| tr!(HISTORY_FULL))
    }

    pub(crate) fn shown(&self) -> impl Iterator<Item = &Entry> {
        self.shown.iter().map(|at| &self.entries[*at])
    }

    pub(crate) fn count(&self) -> usize {
        self.shown.len()
    }

    fn row(&self, index: usize) -> Option<&Entry> {
        self.entries.get(*self.shown.get(index)?)
    }

    pub(crate) fn reread(&mut self) {
        let anchor = self.row(self.selected).map(|entry| entry.id);
        self.entries = self.history.snapshot();
        self.narrow();
        let anchor_index = anchor.and_then(|id| self.shown().position(|entry| entry.id == id));
        self.selected =
            anchor_index.unwrap_or_else(|| self.selected.min(self.count().saturating_sub(1)));
    }

    fn narrow(&mut self) {
        let query = self.query.text().trim().to_lowercase();
        self.shown = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| query.is_empty() || entry.matches(&query))
            .map(|(at, _)| at)
            .collect();
    }

    pub(crate) fn walk(&mut self, delta: i32) {
        if self.shown.is_empty() {
            return;
        }
        let last = self.count() as i32 - 1;
        self.selected = (self.selected as i32 + delta).clamp(0, last) as usize;
    }

    pub(super) fn paste(&self, index: usize) {
        self.put_back(index, PASTE);
    }

    pub(crate) fn copy(&self, index: usize) {
        self.put_back(index, COPY);
    }

    fn put_back(&self, index: usize, method: &'static str) {
        let Some(entry) = self.row(index) else {
            return;
        };
        self.seam
            .notify(Request::new(CAPABILITY, method).encode(&entry.id));
    }

    pub(crate) fn typed(&mut self) {
        self.narrow();
        self.selected = 0;
    }

    pub(crate) fn clear(&mut self) {
        self.history.clear();
        self.reread();
        self.selected = 0;
    }

    pub(crate) fn pin(&mut self, index: usize) {
        let Some(entry) = self.row(index) else {
            return;
        };
        let (id, pinned) = (entry.id, entry.pinned);
        self.history.pin(id, !pinned);
        self.reread();
    }

    pub(crate) fn forget(&mut self, index: usize) {
        let Some(entry) = self.row(index) else {
            return;
        };
        self.history.remove(entry.id);
        self.reread();
    }

    pub(crate) fn escape(&mut self) {
        if self.query.is_empty() {
            return self.close();
        }
        self.query = TextDocument::default();
        self.narrow();
        self.selected = 0;
    }

    pub(crate) fn close(&self) {
        self.seam.notify(Request::new(CAPABILITY, CLOSE));
    }
}
