mod access;
mod amending;
mod conversing;
mod mcp;
mod offers;
mod palette;
mod waiting;
mod worktrees;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use rikka_app::prelude::*;
use rikka_chat::{Draft, Drafts, Models, Offers, Picking, Transcript};
use rikka_llm::Trouble;
use rikka_llm_accounts::Selection;
use rikka_mcp::Hub;
use rikka_terminal::Report;
use rikka_thread::{Attachment, Store, Summary, Thread};
use rikka_ui::search::Action;

use crate::project::act::given_with_setup;
use crate::project::file::Viewer;
use crate::project::git::Git;
use crate::project::handle::ProjectHandle;
use crate::project::palette::Palette;
use crate::project::places::Places;
use crate::project::runner::Runner;
use crate::project::runners::Runners;
use crate::project::tab::Tab;
use crate::project::terminal::Terminal;
use crate::project::terminals::Terminals;
use crate::project::threads::Threads;
use crate::project::tool_call::ToolCallLabels;
use crate::project::workspace::Workspace;
use crate::project::worktree::{Making, canonical_path};
use crate::setup::Setup;

const THREADS: &str = "rikka-ade/projects";

const MINUTE: Duration = Duration::from_secs(60);
const AGE: &str = "ade-minute";

#[derive(Debug, PartialEq)]
struct ThreadError {
    thread: Option<String>,
    trouble: Trouble,
}

struct Offered {
    revision: u64,
    selection: Option<Selection>,
    models: Rc<Models>,
}

#[derive(State)]
pub(crate) struct Project {
    #[untracked]
    root: PathBuf,
    #[untracked]
    here: PathBuf,
    workspaces: Tracked<BTreeMap<PathBuf, Workspace>>,
    #[state]
    runners: Runners,
    #[untracked]
    hub: Hub,
    #[untracked]
    instruction: String,
    #[untracked]
    unreached: BTreeSet<String>,
    #[untracked]
    troubles: Vec<String>,
    #[untracked]
    store: Option<Store>,
    #[state]
    threads: Threads,
    loaded: Tracked<bool>,
    open: Tracked<String>,
    reading: Tracked<Option<String>>,
    pending: Tracked<Option<PathBuf>>,
    making: Tracked<Option<Making>>,
    tab: Tracked<Tab>,
    palette: Tracked<Option<Palette>>,
    draft: Tracked<TextDocument>,
    attached: Tracked<Vec<Attachment>>,
    #[untracked]
    drafts: Drafts,
    #[state]
    offers: Offers,
    picking: Tracked<Picking>,
    #[untracked]
    models: RefCell<Option<Offered>>,
    #[untracked]
    places: Places,
    folded: Tracked<bool>,
    sending: Tracked<bool>,
    #[untracked]
    handle: ProjectHandle,
    #[state]
    terminals: Terminals,
    #[state]
    transcript: Transcript,
    error_message: Tracked<Option<ThreadError>>,
    minute: Tracked<u64>,
}

impl Project {
    pub(crate) fn open(root: PathBuf, setup: &Setup, handle: &ProjectHandle) -> Project {
        let mut project = Project {
            workspaces: Tracked::default(),
            runners: Runners::default(),
            hub: crate::project::hub(Some(&root)),
            instruction: String::new(),
            unreached: BTreeSet::new(),
            troubles: Vec::new(),
            store: None,
            threads: Threads::default(),
            loaded: Tracked::new(false),
            open: Tracked::default(),
            reading: Tracked::new(None),
            pending: Tracked::new(None),
            making: Tracked::new(None),
            tab: Tracked::default(),
            palette: Tracked::new(None),
            draft: Tracked::new(TextDocument::new("")),
            attached: Tracked::default(),
            drafts: Drafts::default(),
            offers: Offers::default(),
            picking: Tracked::default(),
            models: RefCell::default(),
            places: Places::default(),
            folded: Tracked::new(false),
            sending: Tracked::new(false),
            handle: handle.clone(),
            terminals: Terminals::new(handle),
            transcript: Transcript::new(ToolCallLabels),
            error_message: Tracked::new(None),
            minute: Tracked::new(0),
            here: canonical_path(&root),
            root: root.clone(),
        };

        project.follow_servers();
        project.enter(root.clone(), setup);
        recall(root);
        every_keyed(AGE, MINUTE, |project: &mut Project| project.aged());
        project
    }

    fn aged(&mut self) {
        *self.minute += 1;
    }

    pub(crate) fn settle(&mut self) {
        let hand = self.runner().and_then(Runner::hand);
        if hand.is_none() && self.watching().is_some() {
            self.watch_main();
        }
        if let Some(runner) = self.runner_mut() {
            runner.mirror(hand.as_ref());
        }
        let amendable = hand.is_none() && self.is_amendable();
        let amending = amendable || *self.sending;

        let Project {
            threads,
            runners,
            open,
            transcript,
            ..
        } = self;
        let runner = runners.get(open.as_str());
        let (thread, answering) = match &hand {
            Some(hand) => (Some(hand.thread()), hand.answer()),
            None => match runner {
                Some(runner) => (Some(runner.thread()), runner.arriving()),
                None => (threads.get(open.as_str()), None),
            },
        };
        transcript.refresh(thread, answering);
        transcript.allow_amends(amendable);
        if !amending {
            transcript.stop_amending();
        }
    }

    pub(crate) fn workspace(&mut self, root: &Path) -> Option<&mut Workspace> {
        self.workspaces.get_mut(root)
    }

    pub(super) fn shown_workspace(&mut self) -> Option<&mut Workspace> {
        let root = self.shown_root();
        self.workspaces.get_mut(&root)
    }

    pub(super) fn turn_to(&mut self, tab: Tab) {
        self.tab.set(tab);
    }

    pub(super) fn begin(&mut self, setup: &Setup) {
        self.error_message.set(None);
        self.show(String::new(), setup);
    }

    pub(super) fn choose(&mut self, id: String, setup: &Setup) {
        self.error_message.set(None);
        self.show(id, setup);
    }

    pub(super) fn report_error(&mut self, message: Arc<str>, thread: Option<String>) {
        self.report_trouble(Trouble::new(message.as_ref()), thread);
    }

    pub(super) fn report_trouble(&mut self, trouble: Trouble, thread: Option<String>) {
        self.error_message
            .set(Some(ThreadError { thread, trouble }));
    }

    fn put_away(&mut self) {
        self.offers.dismiss();
        self.picking.set(Picking::new(false));
    }

    pub(super) fn fold_screen(&mut self, folded: bool) {
        self.folded.set(folded);
    }

    pub(super) fn open_find(&mut self) {
        self.transcript.open_find();
    }

    pub(super) fn find(&mut self, action: Action) {
        self.transcript.find(action);
    }

    pub(super) fn terminal(&mut self, act: impl FnOnce(&mut Terminal)) {
        self.terminals.act(&self.open, act);
    }

    pub(super) fn raise_terminal(&mut self, act: impl FnOnce(&mut Terminal, &Path)) {
        if self.runner().and_then(Runner::asking).is_some() {
            skip_view();
            return;
        }
        self.put_away();
        let at = self.shown_root();
        self.terminals.raise(&self.open, &at, act);
    }

    pub(super) fn terminal_report(&mut self, serial: u64, report: Report) {
        self.terminals.report(serial, report);
    }

    pub(super) fn viewer(&mut self, act: impl FnOnce(&mut Viewer, &Git)) {
        match self.shown_workspace() {
            Some(workspace) => workspace.file(act),
            None => skip_view(),
        }
    }

    pub(super) fn git(&mut self, act: impl FnOnce(&mut Git)) {
        match self.shown_workspace() {
            Some(workspace) => act(workspace.git_mut()),
            None => skip_view(),
        }
    }

    fn recalled(&mut self, recalled: Result<(Store, Vec<Summary>), String>, setup: &Setup) {
        self.loaded.set(true);
        match recalled {
            Ok((store, listed)) => {
                let handle = self.handle.clone();
                store.watch(move |error_message| {
                    let message: Arc<str> =
                        tr!("ade.chat.unwritable", reason = &error_message.to_string()).into();
                    handle.say(move |project| project.report_error(message, None));
                });
                for thread in self.threads.loaded() {
                    store.keep(thread);
                }
                for runner in self.runners.iter_mut() {
                    runner.keep_in(store.clone());
                }
                self.store = Some(store);
                self.threads.recall(listed);
                if self.open.is_empty() {
                    let first = self.threads.first();
                    self.show(first, setup);
                    return;
                }
                self.read_history();
            }
            Err(reason) => {
                let message = tr!("ade.chat.unwritable", reason = &reason).into();
                self.report_error(message, None);
            }
        }
    }

    fn read_history(&mut self) {
        let id = self.open.to_string();
        if id.is_empty()
            || self.holds(&id)
            || !self.threads.is_listed(&id)
            || self.reading.as_deref() == Some(id.as_str())
        {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        self.reading.set(Some(id.clone()));
        let requested = id.clone();
        spawn_blocking(
            move || read_thread(&store, &requested),
            given_with_setup(move |project, setup, read: Result<Box<Thread>, String>| {
                project.opened(id.clone(), read, setup);
            }),
        );
    }

    fn opened(&mut self, id: String, read: Result<Box<Thread>, String>, setup: &Setup) {
        if self.reading.as_deref() == Some(id.as_str()) {
            self.reading.set(None);
        }
        match read {
            Ok(thread) => {
                if !self.holds(&id) {
                    self.threads.opened(*thread);
                }
                self.settle_selection(&id, setup);
            }
            Err(reason) => {
                let message = tr!("ade.chat.unreadable", reason = &reason).into();
                self.report_error(message, Some(id));
                return;
            }
        }
        self.release_idle_threads();
    }

    fn unmake(&mut self) {
        if self.making.is_some() {
            *self.making = None;
        }
    }

    fn show(&mut self, id: String, setup: &Setup) {
        self.unmake();
        if id == *self.open {
            return;
        }
        self.stash_draft();
        self.settle_selection(&id, setup);
        self.open.set(id);
        self.restore_draft();
        self.transcript.forget();
        self.put_away();
        let root = self.shown_root();
        self.enter(root, setup);
        self.read_history();
        self.release_idle_threads();
    }

    fn stash_draft(&mut self) {
        if !self.draft.is_empty() || !self.attached.is_empty() {
            self.drafts.stash(
                &self.open,
                Draft::new(
                    std::mem::take(&mut *self.draft),
                    std::mem::take(&mut *self.attached),
                ),
            );
        }
    }

    fn restore_draft(&mut self) {
        if let Some(draft) = self.drafts.restore(&self.open) {
            *self.draft = draft.text;
            *self.attached = draft.attached;
        } else {
            self.draft.reset("");
            self.attached.clear();
        }
    }
}
fn read_thread(store: &Store, id: &str) -> Result<Box<Thread>, String> {
    store
        .read(id)
        .map(Box::new)
        .map_err(|error| error.to_string())
}

fn recall(project: PathBuf) {
    spawn_blocking(
        move || {
            let store = Store::open(&kept_in(&project)?).map_err(|error| error.to_string())?;
            let listed = store.summaries();
            Ok((store, listed))
        },
        given_with_setup(
            |project, setup, recalled: Result<(Store, Vec<Summary>), String>| {
                project.recalled(recalled, setup);
            },
        ),
    );
}

fn kept_in(project: &Path) -> Result<PathBuf, String> {
    let home = rikka_dirs::data_home().ok_or_else(|| tr!("ade.chat.homeless").to_string())?;
    Ok(home.join(THREADS).join(rikka_thread::named(project)))
}

impl std::fmt::Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("root", &self.root)
            .field("workspaces", &self.workspaces.len())
            .field("threads", &self.threads.listed().len())
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}
