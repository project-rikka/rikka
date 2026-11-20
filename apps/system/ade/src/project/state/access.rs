use std::path::Path;
use std::rc::Rc;

use rikka_app::prelude::*;
use rikka_chat::{Models, Picking, Transcript};
use rikka_llm::Trouble;
use rikka_llm_accounts::{Llm, Selection};
use rikka_mcp::Hub;
use rikka_thread::{Attachment, Summary, Thread};

use crate::project::places::Places;
use crate::project::runner::Runner;
use crate::project::sidebar;
use crate::project::tab::Tab;
use crate::project::terminal::Terminal;

use super::{Offered, Project};

impl Project {
    pub(in crate::project) fn minute(&self) -> u64 {
        *self.minute
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn hub(&self) -> &Hub {
        &self.hub
    }

    pub(crate) fn heading(&self) -> Option<&str> {
        self.summary()
            .map(Summary::title)
            .filter(|title| !title.is_empty())
    }

    pub(crate) fn place(&self) -> Option<String> {
        let shown = self.shown_root();
        (shown != self.root).then(|| self.named(&shown))
    }

    pub(in crate::project) fn lines(&self) -> Option<Vec<sidebar::Line<'_>>> {
        if !*self.loaded {
            return None;
        }
        let wrong = |id: &str| {
            self.error_message
                .as_ref()
                .is_some_and(|error_message| error_message.thread.as_deref() == Some(id))
        };
        Some(
            self.threads
                .listed()
                .iter()
                .map(|summary| {
                    let runner = self.runners.get(summary.id());
                    sidebar::Line {
                        place: summary.worktree().map(|at| self.named(at)),
                        running: runner.is_some_and(Runner::is_running),
                        asking: runner.and_then(Runner::asking).is_some(),
                        wrong: wrong(summary.id()),
                        thread: summary,
                    }
                })
                .collect(),
        )
    }

    pub(in crate::project) fn thread(&self) -> Option<&Thread> {
        self.thread_of(&self.open)
    }

    pub(in crate::project) fn summary(&self) -> Option<&Summary> {
        self.threads.summary(&self.open)
    }

    pub(super) fn selection_in(&self, llm: &Llm) -> Option<Selection> {
        self.summary()
            .map_or_else(|| llm.selection(), |summary| summary.selection_in(llm))
    }

    pub(in crate::project) fn runner(&self) -> Option<&Runner> {
        self.runners.get(&self.open)
    }

    pub(super) fn runner_mut(&mut self) -> Option<&mut Runner> {
        let Project { runners, open, .. } = self;
        runners.get_mut(open)
    }

    pub(super) fn watching(&self) -> Option<u64> {
        self.runner().and_then(Runner::watching)
    }

    pub(super) fn is_running(&self) -> bool {
        self.runner().is_some_and(Runner::is_running)
    }

    pub(in crate::project) fn error_message(&self) -> Option<&Trouble> {
        self.error_message
            .as_ref()
            .filter(|error_message| {
                error_message
                    .thread
                    .as_deref()
                    .is_none_or(|it| it == self.open.as_str())
            })
            .map(|error_message| &error_message.trouble)
    }

    pub(in crate::project) fn tab(&self) -> Tab {
        *self.tab
    }

    pub(in crate::project) fn open_thread(&self) -> &str {
        &self.open
    }

    pub(in crate::project) fn places(&self) -> &Places {
        &self.places
    }

    pub(in crate::project) fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub(in crate::project) fn is_reading(&self) -> bool {
        self.watching().is_none() && self.reading.as_deref() == Some(self.open.as_str())
    }

    pub(in crate::project) fn draft(&self) -> &TextDocument {
        &self.draft
    }

    pub(in crate::project) fn attached(&self) -> &[Attachment] {
        &self.attached
    }

    pub(in crate::project) fn is_sending(&self) -> bool {
        *self.sending
    }

    pub(in crate::project) fn is_folded(&self) -> bool {
        *self.folded
    }

    pub(in crate::project) fn picking_models(&self) -> Picking {
        *self.picking
    }

    pub(in crate::project) fn models(&self, llm: &Llm) -> Rc<Models> {
        let selection = self.thread().and_then(Thread::selection).cloned();
        let mut cached = self.models.borrow_mut();
        if let Some(offered) = cached.as_ref()
            && offered.revision == llm.revision()
            && offered.selection == selection
        {
            return Rc::clone(&offered.models);
        }
        let models = Rc::new(Models::new(llm, selection.as_ref()));
        *cached = Some(Offered {
            revision: llm.revision(),
            selection,
            models: Rc::clone(&models),
        });
        models
    }

    pub(in crate::project) fn shell(&self) -> Option<&Terminal> {
        self.terminals.get(&self.open)
    }
}
