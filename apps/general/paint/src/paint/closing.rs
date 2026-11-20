use rikka_app::prelude::*;

use super::Paint;
use crate::tab::Tab;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingAction {
    CloseTab,
    CloseWindow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnsavedPrompt {
    tab: u64,
    waiting: Vec<u64>,
    action: PendingAction,
    saving: bool,
}

impl UnsavedPrompt {
    pub(crate) fn tab(&self) -> u64 {
        self.tab
    }

    pub(crate) fn is_saving(&self) -> bool {
        self.saving
    }
}

impl Paint {
    pub(crate) fn unsaved_prompt(&self) -> Option<&UnsavedPrompt> {
        self.unsaved_prompt.as_ref()
    }

    pub(crate) fn prompted_tab(&self) -> Option<&Tab> {
        let unsaved_prompt = self.unsaved_prompt.as_ref()?;
        self.tabs.iter().find(|tab| tab.id() == unsaved_prompt.tab)
    }

    pub(crate) fn close(&mut self, index: usize) {
        if self.unsaved_prompt.is_some() {
            return;
        }
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        match tab.is_modified() {
            true => {
                let id = tab.id();
                self.ask(id, Vec::new(), PendingAction::CloseTab);
            }
            false => drop(self.lift(index)),
        }
    }

    pub(crate) fn close_active(&mut self) {
        self.close(self.active);
    }

    pub(crate) fn close_tab(&mut self, tab: u64) {
        if let Some(index) = self.tabs.iter().position(|candidate| candidate.id() == tab) {
            self.close(index);
        }
    }

    pub(crate) fn ask_to_close_window(&mut self) {
        if self.unsaved_prompt.is_some() {
            return;
        }
        let mut modified: Vec<u64> = self
            .tabs
            .iter()
            .filter(|tab| tab.is_modified())
            .map(Tab::id)
            .collect();
        if modified.is_empty() {
            request_window(WindowRequest::Close);
            return;
        }
        let first = modified.remove(0);
        self.ask(first, modified, PendingAction::CloseWindow);
    }

    fn ask(&mut self, tab: u64, waiting: Vec<u64>, action: PendingAction) {
        if let Some(index) = self.tabs.iter().position(|candidate| candidate.id() == tab) {
            self.active = index;
        }
        self.unsaved_prompt = Some(UnsavedPrompt {
            tab,
            waiting,
            action,
            saving: false,
        });
    }

    pub(crate) fn answer(&mut self, answer: Unsaved) {
        let Some(unsaved_prompt) = &mut self.unsaved_prompt else {
            return;
        };
        if unsaved_prompt.saving {
            return;
        }
        let tab = unsaved_prompt.tab;
        match answer {
            Unsaved::Cancel => self.unsaved_prompt = None,
            Unsaved::Discard => {
                self.drop_tab(tab);
                self.ask_next();
            }
            Unsaved::Save => {
                unsaved_prompt.saving = true;
                self.save_tab(tab);
            }
        }
    }

    pub(super) fn resolve_unsaved_prompt(&mut self, tab: u64, saved: bool) {
        let Some(unsaved_prompt) = &self.unsaved_prompt else {
            return;
        };
        if unsaved_prompt.tab != tab || !unsaved_prompt.saving {
            return;
        }
        match saved {
            true => {
                self.drop_tab(tab);
                self.ask_next();
            }
            false => self.unsaved_prompt = None,
        }
    }

    pub(super) fn abandon_question(&mut self, tab: u64) {
        if self
            .unsaved_prompt
            .as_ref()
            .is_some_and(|unsaved_prompt| unsaved_prompt.tab == tab)
        {
            self.unsaved_prompt = None;
        }
    }

    fn drop_tab(&mut self, tab: u64) {
        if let Some(index) = self.tabs.iter().position(|candidate| candidate.id() == tab) {
            drop(self.lift(index));
        }
    }

    fn ask_next(&mut self) {
        let Some(mut unsaved_prompt) = self.unsaved_prompt.take() else {
            return;
        };
        while !unsaved_prompt.waiting.is_empty() {
            let next = unsaved_prompt.waiting.remove(0);
            if self
                .tabs
                .iter()
                .any(|tab| tab.id() == next && tab.is_modified())
            {
                self.ask(next, unsaved_prompt.waiting, unsaved_prompt.action);
                return;
            }
        }
        if unsaved_prompt.action == PendingAction::CloseWindow {
            request_window(WindowRequest::Close);
        }
    }
}
