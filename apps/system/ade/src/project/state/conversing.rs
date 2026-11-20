use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_chat::{Attached, Builtin, Gathered, Picking, gather, invoked, mentioned, needs_mcp};
use rikka_llm::Effort;
use rikka_llm_accounts::{AccountId, Selection};
use rikka_thread::{Ask, Attachment, ConnectedModel, DeskEvent, Reply, RunOutcome, Thread};

use crate::labels;
use crate::project::act::given_with_setup;
use crate::project::attaching;
use crate::project::catalogs::Catalogs;
use crate::project::chat::AdeChat;
use crate::project::machine_input::MachineInput;
use crate::project::runner::{Fitted, Runner};
use crate::project::workspace::Workspace;
use crate::setup::Setup;

use super::Project;

struct Collected {
    root: PathBuf,
    ask: Ask,
    catalogs: Option<Catalogs>,
    mentioned: Attached,
}

pub(super) enum StartError {
    Silent,
    Message(Arc<str>),
}

fn unready(error: &rikka_thread::Error) -> Arc<str> {
    tr!("ade.chat.unready", reason = &error.to_string()).into()
}

impl Project {
    pub(in crate::project) fn watch_agent(&mut self, job: u64) {
        self.put_away();
        if let Some(runner) = self.runner_mut() {
            runner.watch_agent(job);
        }
    }

    pub(in crate::project) fn watch_main(&mut self) {
        if let Some(runner) = self.runner_mut() {
            runner.watch_main();
        }
    }

    pub(crate) fn agent_said(&mut self, thread: &str) {
        if !self.agent_streamed(thread) {
            self.settle();
        }
    }

    fn agent_streamed(&mut self, thread: &str) -> bool {
        if *self.open != thread {
            skip_view();
            return true;
        }
        let Some(hand) = self.runner().and_then(Runner::hand) else {
            return false;
        };
        if !self.transcript.grow(Some(hand.thread()), hand.answer()) {
            return false;
        }
        skip_view();
        true
    }

    pub(in crate::project) fn stop_job(&mut self, id: u64) {
        if let Some(runner) = self.runner() {
            runner.stop_job(id);
        }
        skip_view();
    }

    pub(in crate::project) fn desk_event(&mut self, thread: &str, event: DeskEvent) {
        let Some(runner) = self.runners.get_mut(thread) else {
            skip_view();
            return;
        };
        let moved = event == DeskEvent::JobsMoved;
        runner.heard(event);
        if moved && runner.is_woken() {
            let thread = thread.to_owned();
            self.handle
                .say_with_setup(move |project, setup| project.woke(&thread, setup));
        }
    }

    pub(in crate::project) fn stop(&mut self) {
        let Some(runner) = self.runner_mut() else {
            return;
        };
        if let Some(job) = runner.watching()
            && runner.asking().is_none()
        {
            runner.stop_job(job);
            skip_view();
            return;
        }
        let unheard = runner.stop();
        self.redraft(unheard);
    }

    pub(in crate::project) fn pick_option(&mut self, option: usize) {
        if let Some(runner) = self.runner_mut() {
            runner.choose(option);
        }
    }

    pub(in crate::project) fn wrote(&mut self) {
        if let Some(runner) = self.runner_mut() {
            runner.wrote();
        }
    }

    pub(in crate::project) fn back(&mut self) {
        if let Some(runner) = self.runner_mut() {
            runner.back();
        }
    }

    pub(in crate::project) fn ahead(&mut self) {
        if let Some(runner) = self.runner_mut() {
            runner.ahead();
        }
    }

    pub(in crate::project) fn forward_input(&mut self, input: MachineInput) {
        match self.runner_mut() {
            Some(runner) => runner.queue_input(input),
            None => skip_view(),
        }
    }

    pub(in crate::project) fn with_runner(&mut self, thread: &str, act: impl FnOnce(&mut Runner)) {
        match self.runners.get_mut(thread) {
            Some(runner) => act(runner),
            None => skip_view(),
        }
    }

    pub(in crate::project) fn model(&mut self, account: AccountId, model: &str, setup: &mut Setup) {
        let effort = self
            .thread()
            .and_then(Thread::selection)
            .map_or(setup.llm.effort(), Selection::effort);
        let selection = Selection::new(account, model, effort);
        setup.llm.select(&selection);
        self.select(selection);
        self.picking.set(Picking::new(false));
        self.error_message.set(None);
    }

    pub(in crate::project) fn effort(&mut self, effort: Effort, setup: &mut Setup) {
        if let Some(selection) = self.selection_in(&setup.llm) {
            self.select(Selection::new(
                selection.account(),
                selection.model(),
                effort,
            ));
        }
        setup.llm.set_effort(effort);
        self.picking.set(Picking::new(false));
    }

    pub(in crate::project) fn picking(&mut self, open: bool) {
        self.picking.set(Picking::new(open));
    }

    pub(in crate::project) fn reasoning(&mut self, at: Option<f32>) {
        let unfolded = self.picking.with_reasoning(at);
        self.picking.set(unfolded);
    }

    pub(super) fn settle_selection(&mut self, id: &str, setup: &Setup) {
        let Some(thread) = self.thread_of(id) else {
            return;
        };
        if thread.selection().is_some() {
            return;
        }
        let Some(selection) = self
            .threads
            .summary(id)
            .and_then(|summary| summary.selection_in(&setup.llm))
        else {
            return;
        };
        if let Some(thread) = self.thread_mut_of(id) {
            thread.select(selection);
        }
        self.keep(id);
    }

    pub(super) fn release_idle_threads(&mut self) {
        let Self { threads, open, .. } = self;
        threads.release_unless(|id| id == open.as_str());
    }

    pub(in crate::project) fn send(&mut self, setup: &mut Setup) {
        if *self.sending || self.watching().is_some() {
            return;
        }
        self.offers.dismiss();

        let text = self.draft.text();
        if let Some(invoked) = invoked(&text)
            && let Some(builtin) = Builtin::named(invoked.name())
        {
            if self.is_running() {
                let message = tr!("ade.chat.busy", command = invoked.name());
                self.report_error(message.into(), None);
                return;
            }
            match builtin {
                Builtin::Compact => self.compact(setup, invoked.arguments()),
            }
            return;
        }

        let ask = self.expanding(text.trim().to_owned());
        if ask.sent_text().trim().is_empty() && self.attached.is_empty() {
            return;
        }

        if needs_mcp(&self.hub, &ask) {
            self.sending.set(true);
            let hub = self.hub.clone();
            spawn(
                async move { gather::<AdeChat>(&hub, ask).await },
                given_with_setup(|project, setup, gathered: Result<Gathered, String>| {
                    project.fetched(gathered, setup);
                }),
            );
            return;
        }
        let mentions = mentioned(ask.sent_text());
        if mentions.is_empty() && self.is_running() {
            self.ask(setup, ask, Vec::new());
            return;
        }
        self.collect(ask, Attached::default(), mentions);
    }

    fn fetched(&mut self, gathered: Result<Gathered, String>, setup: &mut Setup) {
        match gathered {
            Ok(Gathered {
                ask,
                attached,
                mentions,
            }) => {
                if mentions.is_empty() && self.is_running() {
                    self.sending.set(false);
                    self.asked(setup, ask, attached);
                    return;
                }
                self.collect(ask, attached, mentions);
            }
            Err(reason) => {
                self.sending.set(false);
                self.report_error(Arc::from(reason), None);
            }
        }
    }

    fn collect(&mut self, ask: Ask, attached: Attached, mentions: Vec<String>) {
        self.sending.set(true);
        let running = self.is_running();
        let root = self.selected_root();
        spawn_blocking(
            move || Collected {
                catalogs: (!running).then(|| Catalogs::find(&root)),
                mentioned: match mentions.is_empty() {
                    true => attached,
                    false => attached.join(attaching::mentioned(&root, &mentions)),
                },
                ask,
                root,
            },
            given_with_setup(|project, setup, collected: Collected| {
                project.collected(collected, setup);
            }),
        );
    }

    fn collected(&mut self, collected: Collected, setup: &mut Setup) {
        let Collected {
            root,
            ask,
            catalogs,
            mentioned,
        } = collected;
        if let Some(catalogs) = catalogs {
            self.catalogued(&root, catalogs, setup);
        }
        self.sending.set(false);
        self.asked(setup, ask, mentioned);
    }

    fn asked(&mut self, setup: &mut Setup, ask: Ask, mentioned: Attached) {
        self.ask(setup, ask, mentioned.attachments);
        if let Some(reason) = mentioned.refused
            && self.error_message.is_none()
        {
            self.report_error(Arc::from(reason), None);
        }
    }

    fn ask(&mut self, setup: &mut Setup, ask: Ask, mentioned: Vec<Attachment>) {
        if ask.sent_text().trim().is_empty() && self.attached.is_empty() && mentioned.is_empty() {
            return;
        }
        let mut attached = (*self.attached).clone();
        attached.extend(mentioned);
        let ask = ask.with_attachments(attached);

        let sent = match self.is_running() {
            true => self.interject(ask),
            false => self.converse(ask, setup),
        };
        match sent {
            Ok(()) => {
                self.error_message.set(None);
                self.draft.reset("");
                self.attached.clear();
            }
            Err(start_failed) => self.start_failed(start_failed),
        }
    }

    fn compact(&mut self, setup: &mut Setup, focus: &str) {
        if !self.summary().is_some_and(|summary| summary.turns() > 0) {
            self.report_error(tr!("ade.chat.unfoldable").into(), None);
            return;
        }
        match self.fold(focus, setup) {
            Ok(()) => {
                self.error_message.set(None);
                self.draft.reset("");
            }
            Err(start_failed) => self.start_failed(start_failed),
        }
    }

    pub(super) fn converse(&mut self, ask: Ask, setup: &Setup) -> Result<(), StartError> {
        let (id, hired) = self.prepare(setup)?;
        let Some(runner) = self.runners.get_mut(&id) else {
            return Err(StartError::Silent);
        };
        runner
            .ask(hired, ask)
            .map_err(|error| StartError::Message(unready(&error)))?;
        self.list(&id);
        Ok(())
    }

    fn fold(&mut self, focus: &str, setup: &Setup) -> Result<(), StartError> {
        let (id, hired) = self.prepare(setup)?;
        let Some(runner) = self.runners.get_mut(&id) else {
            return Err(StartError::Silent);
        };
        runner
            .compact(hired, focus)
            .map_err(|error| StartError::Message(unready(&error)))
    }

    fn prepare(&mut self, setup: &Setup) -> Result<(String, ConnectedModel), StartError> {
        let root = self.selected_root();
        if !root.is_dir() {
            let message = tr!(
                "ade.chat.worktree.gone",
                worktree = &labels::file_name(&root)
            );
            return Err(StartError::Message(message.into()));
        }

        if !self.enter(root.clone(), setup) {
            self.refresh(&root, setup);
        }
        let ready = self
            .workspaces
            .get(&root)
            .is_some_and(|workspace| workspace.assistant().is_some());
        if !ready {
            return Err(StartError::Silent);
        }

        let open = self.open.to_string();
        let mut fresh = (!self.holds(&open)).then(|| self.fresh_at(&root));
        let hired = match fresh.as_mut() {
            Some(thread) => thread.connect_model(&setup.llm),
            None => match self.thread_mut_of(&open) {
                Some(thread) => thread.connect_model(&setup.llm),
                None => return Err(StartError::Silent),
            },
        }
        .map_err(|error| StartError::Message(labels::account_error(&error)))?;
        if let Some(thread) = fresh {
            self.begin_with(thread);
        }

        let id = self.open.to_string();
        self.attach_runner(&id, &root)?;
        Ok((id, hired))
    }

    pub(super) fn start_failed(&mut self, error: StartError) {
        if let StartError::Message(message) = error {
            self.report_error(message, None);
        }
    }
    pub(super) fn attach_runner(&mut self, thread: &str, root: &Path) -> Result<(), StartError> {
        let built = self.workspaces.get(root).map_or(0, Workspace::built);
        let has_runner = self.runners.get(thread).is_some_and(|runner| {
            runner.root() == root && (runner.built() == built || runner.is_occupied())
        });
        if has_runner {
            return Ok(());
        }

        let Some(made) = self
            .workspaces
            .get(root)
            .and_then(Workspace::assistant)
            .map(|assistant| assistant.fresh())
        else {
            return Err(StartError::Silent);
        };
        let fitted = Fitted {
            root: root.to_owned(),
            built,
            assistant: made.map_err(|error| StartError::Message(unready(&error)))?,
        };
        if let Some(runner) = self.runners.get_mut(thread) {
            if let Some(ended) = runner.equip(fitted) {
                spawn_blocking(move || drop(ended), |(): ()| skip_view());
            }
            return Ok(());
        }
        let Some(held) = self.threads.take(thread) else {
            return Err(StartError::Silent);
        };
        let runner = Runner::open(held, fitted, self.store.clone(), &self.handle);
        self.runners.insert(thread.to_owned(), runner);
        Ok(())
    }

    pub(super) fn select(&mut self, selection: Selection) {
        let root = self.selected_root();
        let thread = self.open_at(&root);
        let Some(thread_state) = self.thread_mut_of(&thread) else {
            return;
        };
        if thread_state.selection() != Some(&selection) {
            thread_state.select(selection);
            self.keep(&thread);
        }
    }

    fn open_at(&mut self, root: &Path) -> String {
        if self.holds(&self.open) {
            return self.open.to_string();
        }
        let thread = self.fresh_at(root);
        self.begin_with(thread)
    }

    fn fresh_at(&self, root: &Path) -> Thread {
        let mut thread = Thread::new();
        thread.set_worktree((root != self.root).then(|| root.to_owned()));
        thread
    }

    fn begin_with(&mut self, thread: Thread) -> String {
        let id = thread.id().to_owned();
        self.terminals.rekey(&self.open, &id);
        self.open.set(id.clone());
        self.threads.begin(thread);
        id
    }

    pub(crate) fn replied(&mut self, thread: &str, reply: Reply) {
        if let Reply::CompactFailed(reason) = &reply {
            tracing::warn!(%reason, "failed to compact conversation");
        }
        let shown = *self.open == thread && self.runner().and_then(Runner::hand).is_none();
        let Some(runner) = self.runners.get_mut(thread) else {
            skip_view();
            return;
        };
        match runner.apply(&reply) {
            RunOutcome::AnswerUpdated if matches!(reply, Reply::Delta(_)) => {
                if !shown {
                    return skip_view();
                }
                let arriving = self.runners.get(thread).and_then(Runner::arriving);
                match self.transcript.stream(arriving) {
                    true => skip_view(),
                    false => self.settle(),
                }
            }
            RunOutcome::AnswerUpdated => self.settle(),
            RunOutcome::ThreadUpdated => {
                self.list(thread);
                self.settle();
            }
            RunOutcome::Finished => {
                let carried_on = matches!(reply, Reply::Done);
                if let Reply::Failed(trouble) = reply {
                    self.report_trouble(trouble, Some(thread.to_owned()));
                }
                self.list(thread);
                self.unheard(thread, carried_on);
                self.wake(thread);
                self.settle();
            }
        }
    }

    fn wake(&mut self, thread: &str) {
        if self.runners.get(thread).is_some_and(Runner::is_woken) {
            let thread = thread.to_owned();
            self.handle
                .say_with_setup(move |project, setup| project.woke(&thread, setup));
        }
    }

    pub(in crate::project) fn woke(&mut self, thread: &str, setup: &Setup) {
        let Some(runner) = self.runners.get(thread) else {
            skip_view();
            return;
        };
        if !runner.is_woken() {
            return;
        }
        if let Err(message) = self.start_in(thread, setup, Runner::wake) {
            self.report_error(message, Some(thread.to_owned()));
        }
    }

    pub(super) fn start_in(
        &mut self,
        thread: &str,
        setup: &Setup,
        begin: impl FnOnce(&mut Runner, ConnectedModel) -> rikka_thread::Result<bool>,
    ) -> Result<bool, Arc<str>> {
        let Some(runner) = self.runners.get_mut(thread) else {
            return Ok(false);
        };
        let hired = runner
            .connect(&setup.llm)
            .map_err(|error| labels::account_error(&error))?;
        if !begin(runner, hired).map_err(|error| unready(&error))? {
            return Ok(false);
        }
        self.list(thread);
        Ok(true)
    }

    pub(in crate::project) fn delete(&mut self, thread: String) {
        if let Some(mut runner) = self.runners.remove(&thread) {
            runner.halt();
            let desk = runner.into_desk();
            spawn_blocking(move || drop(desk), |(): ()| skip_view());
        }
        self.forget(thread);
    }

    pub(super) fn forget(&mut self, thread: String) {
        if let Some(store) = self.store.as_ref() {
            store.discard(&thread);
        }
        self.threads.forget(&thread);
        self.drafts.forget(&thread);
        self.terminals.close(&thread);
        if *self.open == thread {
            let first = self.threads.first();
            self.open.set(first);
            self.restore_draft();
            self.transcript.forget();
            self.read_history();
        }
    }

    pub(super) fn keep(&mut self, id: &str) {
        match self.runners.get(id) {
            Some(runner) => {
                runner.keep();
                self.list(id);
            }
            None => self.threads.keep(id, self.store.as_ref()),
        }
    }

    fn list(&mut self, id: &str) {
        if let Some(runner) = self.runners.get(id) {
            let summary = runner.thread().summary();
            self.threads.list(summary);
        }
    }

    pub(super) fn holds(&self, id: &str) -> bool {
        self.runners.get(id).is_some() || self.threads.holds(id)
    }

    pub(super) fn thread_of(&self, id: &str) -> Option<&Thread> {
        match self.runners.get(id) {
            Some(runner) => Some(runner.thread()),
            None => self.threads.get(id),
        }
    }

    pub(super) fn thread_mut_of(&mut self, id: &str) -> Option<&mut Thread> {
        let Project {
            runners, threads, ..
        } = self;
        match runners.get_mut(id) {
            Some(runner) => Some(runner.thread_mut()),
            None => threads.get_mut(id),
        }
    }
}
