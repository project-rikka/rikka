use std::sync::Arc;

use futures_util::{Stream, StreamExt as _};
use rikka_agent::ToolKitSet;
use rikka_assist::Apps;
use rikka_chat::Models;
use rikka_computer_use::ComputerUse;
use rikka_llm::Trouble;
use rikka_llm_accounts::{ConnectedModel, Llm, Selection};
use rikka_mcp::Hub;
use rikka_seam::SeamClient;
use rikka_sync::{Mutex, MutexGuard};
use rikka_thread::{
    Answer, Ask, Assistant, Desk, DeskEvent, Replies, Reply, RunOutcome, Store, Summary, Thread,
};
use rikka_ui::tr;
use rikka_worker::Observable;
use tokio::sync::mpsc;

use crate::fitting;
use crate::prompts::Prompts;
use crate::shown::{Readiness, Shown};
use crate::turn_context::{ReportedContext, TurnContext};

#[derive(Clone, Debug)]
pub(crate) struct Assistance {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    seam: SeamClient,
    shelf: ToolKitSet,
    apps: Apps,
    hub: Hub,
    prompts: Result<Arc<Prompts>, Arc<str>>,
    state: Mutex<AssistanceState>,
    moves: Observable<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LayoutChange {
    Moved,
    Grew,
}

#[derive(Debug)]
struct AssistanceState {
    llm: Llm,
    desk: Desk,
    store: Option<Store>,
    summaries: Arc<[Summary]>,
    trouble: Option<Trouble>,
    readiness: Readiness,
    language: String,
    instructed: String,
    reported: ReportedContext,
    summoned: Option<String>,
    opening: u64,
    fitting: bool,
    refit_due: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fit {
    Fresh,
    Again,
}

struct Fitted {
    made: Result<Assistant, Arc<str>>,
    language: String,
    instructed: String,
}

type Begin<'a> = Box<
    dyn FnOnce(
            &mut Desk,
            ConnectedModel,
            &mut ReportedContext,
        ) -> rikka_thread::Result<Option<Replies>>
        + 'a,
>;

impl AssistanceState {
    fn open(&mut self, thread: Thread) {
        let leaving = self.desk.open(thread).summary();
        if leaving.turns() > 0 {
            self.summaries = beside(&self.summaries, leaving);
        }
        self.reported = ReportedContext::default();
    }

    fn is_busy(&self) -> bool {
        self.desk.is_answering()
            || self.desk.assistant().is_some_and(|assistant| {
                !assistant.notices().is_empty() || assistant.jobs().running() > 0
            })
    }

    fn reject_if_busy(&mut self) -> bool {
        let busy = self.is_busy();
        if busy {
            self.trouble = Some(Trouble::new(tr!("shell.assistant.busy").as_str()));
        }
        busy
    }

    fn take_refit(&mut self) -> bool {
        let should_refit = self.refit_due && !self.fitting && !self.is_busy();
        if should_refit {
            self.refit_due = false;
            self.fitting = true;
        }
        should_refit
    }

    fn begin_fitting(&mut self) -> bool {
        let begun = !self.fitting;
        self.fitting = true;
        begun
    }

    fn install(&mut self, fit: Fit, fitted: Fitted) -> Option<Assistant> {
        self.fitting = false;
        let assistant = match fitted.made {
            Ok(assistant) => assistant,
            Err(error) => {
                self.trouble = Some(Trouble::new(error.as_ref()));
                if fit == Fit::Fresh {
                    self.readiness = Readiness::Unready;
                }
                return None;
            }
        };
        if self.is_busy() {
            self.refit_due = true;
            return Some(assistant);
        }
        self.language = fitted.language;
        self.instructed = fitted.instructed;
        self.readiness = Readiness::Ready;
        self.desk.equip(assistant)
    }

    fn start(&mut self, begin: Begin<'_>) -> Option<Replies> {
        if self.desk.is_answering() || self.desk.assistant().is_none() {
            return None;
        }
        let hired = match self.desk.connect(&self.llm) {
            Ok(hired) => hired,
            Err(error) => {
                self.trouble = Some(Trouble::new(error.to_string()));
                return None;
            }
        };
        match begin(&mut self.desk, hired, &mut self.reported) {
            Ok(replies) => {
                self.trouble = None;
                replies
            }
            Err(error) => {
                self.trouble = Some(Trouble::new(error.to_string()));
                None
            }
        }
    }
}

impl Assistance {
    pub(crate) fn open(seam: SeamClient) -> Assistance {
        let shelf = ToolKitSet::new();
        let hub = fitting::servers();
        let prompts = Prompts::load()
            .map(Arc::new)
            .map_err(|error| Arc::from(error.to_string()));
        let (heard, mut events) = mpsc::unbounded_channel();
        let mut desk = Desk::new(Thread::new());
        desk.watch(move |event| {
            let _sent = heard.send(event);
        });
        let assistance = Assistance {
            inner: Arc::new(Inner {
                seam,
                apps: Apps::watch(shelf.clone()),
                shelf,
                hub,
                prompts,
                state: Mutex::new(AssistanceState {
                    llm: Llm::load(),
                    desk,
                    store: None,
                    summaries: Arc::from([]),
                    trouble: None,
                    readiness: Readiness::Opening,
                    language: String::new(),
                    instructed: String::new(),
                    reported: ReportedContext::default(),
                    summoned: None,
                    opening: 0,
                    fitting: true,
                    refit_due: false,
                }),
                moves: Observable::new(0),
            }),
        };
        assistance.follow_accounts();
        assistance.follow_servers();

        let hearing = assistance.clone();
        fitting::spawn(async move {
            while let Some(event) = events.recv().await {
                hearing.heard(event);
            }
        });

        let opening = assistance.clone();
        fitting::spawn_blocking(move || opening.fit());
        assistance
    }

    fn follow_accounts(&self) {
        let Some(changes) = self.state().llm.changes() else {
            return;
        };
        let following = self.clone();
        fitting::spawn(async move {
            let mut changes = std::pin::pin!(changes);
            while let Some(announced) = changes.next().await {
                let restated = following.edit(|state| {
                    state.llm.apply_change(announced);
                    state.llm.reply_language() != state.language
                });
                if restated {
                    following.refit();
                }
            }
        });
    }

    fn follow_servers(&self) {
        let following = self.clone();
        fitting::spawn(async move {
            let hub = following.inner.hub.clone();
            let mut changes = std::pin::pin!(hub.changes());
            hub.start().await;
            loop {
                let instruction = hub.instruction();
                if following.state().instructed != instruction {
                    following.refit();
                }
                if changes.next().await.is_none() {
                    break;
                }
            }
        });
    }

    fn refit(&self) {
        let should_refit = {
            let mut state = self.state();
            state.refit_due = true;
            state.take_refit()
        };
        if should_refit {
            let refitting = self.clone();
            fitting::spawn_blocking(move || refitting.fit_assistant(Fit::Fresh));
        }
    }

    fn settle(&self) {
        if self.state().take_refit() {
            let refitting = self.clone();
            fitting::spawn_blocking(move || refitting.fit_assistant(Fit::Fresh));
        }
    }

    fn heard(&self, event: DeskEvent) {
        self.edit(|state| state.desk.heard(event));
    }

    pub(crate) fn shown(&self) -> Shown {
        let state = self.state();
        let thread = Arc::clone(state.desk.thread());
        Shown {
            readiness: state.readiness,
            answering: state.desk.answer().cloned(),
            summaries: Arc::clone(&state.summaries),
            surveys: state.desk.questions().to_vec(),
            trouble: state.trouble.clone(),
            jobs: state.desk.jobs().to_vec(),
            selection: thread.selection_in(&state.llm),
            model: state.llm.model_of(thread.selection()).cloned(),
            thread,
        }
    }

    pub(crate) fn models(&self, chosen: Option<&Selection>) -> Models {
        Models::new(&self.state().llm, chosen)
    }

    pub(crate) fn apps(&self) -> &Apps {
        &self.inner.apps
    }

    pub(super) fn hub(&self) -> &Hub {
        &self.inner.hub
    }

    pub(super) fn summon(&self, kit: String) {
        self.edit(|state| state.summoned = Some(kit));
    }

    pub(super) fn take_summoned(&self) -> Option<String> {
        self.state().summoned.take()
    }

    pub(super) fn beats(&self) -> impl Stream<Item = LayoutChange> + Send + 'static {
        let last_moves = self.inner.moves.get();
        self.inner
            .moves
            .changes()
            .scan(last_moves, |last_moves, moves| {
                let stirred = match moves == *last_moves {
                    true => LayoutChange::Grew,
                    false => LayoutChange::Moved,
                };
                *last_moves = moves;
                std::future::ready(Some(stirred))
            })
    }

    pub(crate) fn answering<R>(&self, read: impl FnOnce(&Answer) -> R) -> Option<R> {
        let state = self.state();
        Some(read(state.desk.answer()?))
    }

    pub(super) fn send_message(&self, ask: Ask) -> bool {
        if ask.sent_text().trim().is_empty() && ask.attachments().is_empty() {
            return false;
        }
        let running = self.inner.apps.running();
        let prompts = self.inner.prompts.as_ref().ok().map(Arc::clone);
        self.begin(Box::new(move |desk, hired, reported| {
            let Some(assistant) = desk.assistant() else {
                return Ok(None);
            };
            let mut updated = reported.clone();
            let turn_context = match &prompts {
                Some(prompts) => {
                    updated.next_turn(prompts, ask.text(), &running, |kit| assistant.toolkit(kit))
                }
                None => TurnContext::default(),
            };
            let replies = desk.ask(hired, ask.with_context(turn_context.context))?;
            if let Some(assistant) = desk.assistant() {
                for kit in &turn_context.opened_kits {
                    let _opened = assistant.open_toolkit(kit);
                }
            }
            *reported = updated;
            Ok(Some(replies))
        }))
    }

    pub(crate) fn compact(&self, focus: &str) -> bool {
        let unsaid = self.edit(|state| {
            let unsaid = state.desk.thread().turns().is_empty();
            if unsaid {
                state.trouble = Some(Trouble::new(tr!("shell.assistant.unfoldable").as_str()));
            }
            unsaid
        });
        if unsaid {
            return false;
        }
        let focus = focus.to_owned();
        self.begin(Box::new(move |desk, hired, _| {
            desk.compact(hired, &focus).map(Some)
        }))
    }

    pub(crate) fn show_error(&self, message: impl Into<Arc<str>>) {
        let trouble = Trouble::new(message.into().as_ref());
        self.edit(|state| state.trouble = Some(trouble));
    }

    pub(crate) fn stop(&self) {
        self.edit(|state| {
            let _unheard = state.desk.stop();
        });
    }

    pub(super) fn begin_thread(&self) {
        self.switch(|_| Some(Thread::new()));
    }

    pub(super) fn open_thread(&self, id: &str) {
        if self.state().desk.thread().id() == id {
            return;
        }
        let id = id.to_owned();
        self.switch(move |store| {
            store?
                .read(&id)
                .inspect_err(|error| tracing::warn!("failed to read conversation {id}: {error}"))
                .ok()
        });
    }

    fn switch(&self, read: impl FnOnce(Option<&Store>) -> Option<Thread> + Send + 'static) {
        let opening_store = self.edit(|state| {
            if state.reject_if_busy() {
                return None;
            }
            state.opening = state.opening.wrapping_add(1);
            Some((state.store.clone(), state.opening))
        });
        let Some((store, opening)) = opening_store else {
            return;
        };
        let assistance = self.clone();
        fitting::spawn_blocking(move || {
            if let Some(thread) = read(store.as_ref())
                && assistance.finish_opening(opening, thread)
            {
                assistance.fit_assistant(Fit::Again);
            }
        });
    }

    fn finish_opening(&self, opening: u64, thread: Thread) -> bool {
        self.edit(|state| {
            if state.opening != opening || state.reject_if_busy() {
                return false;
            }
            state.open(thread);
            state.trouble = None;
            state.begin_fitting()
        })
    }

    pub(super) fn discard_thread(&self, id: &str) -> bool {
        let discarded = self.edit(|state| {
            let open = state.desk.thread().id() == id;
            if open && state.reject_if_busy() {
                return None;
            }
            if let Some(store) = &state.store {
                store.discard(id);
            }
            state.summaries = state
                .summaries
                .iter()
                .filter(|summary| summary.id() != id)
                .cloned()
                .collect();
            if open {
                let _discarded = state.desk.open(Thread::new());
                state.reported = ReportedContext::default();
            }
            Some(open)
        });
        if discarded == Some(true) {
            self.begin_thread();
        }
        discarded.is_some()
    }

    pub(crate) fn choose(&self, selection: Selection) {
        self.edit(|state| {
            state.llm.select(&selection);
            state.desk.thread_mut().select(selection);
            state.desk.keep();
            state.trouble = None;
        });
    }

    pub(super) fn answer_question(&self, id: u64, selections: Vec<rikka_thread::Selection>) {
        self.edit(|state| state.desk.answer_question(id, selections));
    }

    pub(super) fn stop_job(&self, id: u64) {
        self.state().desk.interrupt_job(id);
    }

    fn fit(&self) {
        let store = fitting::store();
        let summaries = store
            .as_ref()
            .map(|store| store.summaries())
            .unwrap_or_default();
        let thread = summaries
            .first()
            .and_then(|summary| store.as_ref()?.read(summary.id()).ok())
            .unwrap_or_default();

        self.edit(|state| {
            state.summaries = summaries.into();
            state.open(thread);
            if let Some(store) = &store {
                state.desk.keep_in(store.clone());
            }
            state.store = store;
        });
        self.fit_assistant(Fit::Fresh);
    }

    fn fit_assistant(&self, first: Fit) {
        let mut fit = first;
        loop {
            let fitted = match fit {
                Fit::Fresh => self.fresh(),
                Fit::Again => self.retry(),
            };
            let (unequipped, due) =
                self.edit(|state| (state.install(fit, fitted), state.take_refit()));
            drop(unequipped);
            if !due {
                return;
            }
            fit = Fit::Fresh;
        }
    }

    fn fresh(&self) -> Fitted {
        let language = self.state().llm.reply_language().to_owned();
        let instructed = self.inner.hub.instruction();
        let made = match &self.inner.prompts {
            Ok(prompts) => fitting::fit(
                prompts,
                self.inner.seam.clone(),
                &language,
                self.inner.shelf.clone(),
                &self.inner.hub,
            )
            .map_err(|error| Arc::from(error.to_string())),
            Err(error) => Err(Arc::clone(error)),
        };
        Fitted {
            made,
            language,
            instructed,
        }
    }

    fn retry(&self) -> Fitted {
        let state = self.state();
        let Some(assistant) = state.desk.assistant() else {
            drop(state);
            return self.fresh();
        };
        Fitted {
            made: assistant
                .fresh()
                .map_err(|error| Arc::from(error.to_string())),
            language: state.language.clone(),
            instructed: state.instructed.clone(),
        }
    }

    fn begin(&self, begin: Begin<'_>) -> bool {
        let Some(mut replies) = self.edit(|state| state.start(begin)) else {
            return false;
        };

        let assistance = self.clone();
        fitting::spawn(async move {
            while let Some(reply) = replies.next().await {
                assistance.arrived(reply);
            }
        });
        true
    }

    fn arrived(&self, reply: Reply) {
        let landed = self.state().desk.apply(&reply);
        match landed {
            RunOutcome::AnswerUpdated => match reply {
                Reply::Responded(_) => {}
                Reply::Delta(_) => self.notify_grown(),
                _ => self.activity_monitor(),
            },
            RunOutcome::ThreadUpdated => self.activity_monitor(),
            RunOutcome::Finished => {
                self.edit(|state| {
                    if let Reply::Failed(reason) = &reply {
                        state.trouble = Some(reason.clone());
                    }
                });
                self.release_screen();
                self.woken();
                self.settle();
            }
        }
    }

    fn woken(&self) {
        if self.state().desk.is_woken() {
            self.begin(Box::new(|desk, hired, _| desk.wake(hired)));
        }
    }

    fn release_screen(&self) {
        let computer = self
            .state()
            .desk
            .assistant()
            .and_then(|assistant| assistant.machine())
            .and_then(ComputerUse::computer_of)
            .cloned();
        if let Some(computer) = computer
            && let Err(error) = computer.show(false)
        {
            tracing::debug!("failed to hide agent pointer: {error}");
        }
    }

    fn edit<T>(&self, edit: impl FnOnce(&mut AssistanceState) -> T) -> T {
        let answered = {
            let mut state = self.state();
            edit(&mut state)
        };
        self.activity_monitor();
        answered
    }

    fn activity_monitor(&self) {
        self.inner.moves.modify(|moves| {
            *moves = moves.wrapping_add(1);
            true
        });
    }

    fn notify_grown(&self) {
        self.inner.moves.modify(|_| true);
    }

    fn state(&self) -> MutexGuard<'_, AssistanceState> {
        self.inner.state.lock()
    }
}

fn beside(summaries: &[Summary], current: Summary) -> Arc<[Summary]> {
    match summaries
        .iter()
        .position(|summary| summary.id() == current.id())
    {
        Some(at) => {
            let mut renewed = summaries.to_vec();
            renewed[at] = current;
            renewed.into()
        }
        None => std::iter::once(current)
            .chain(summaries.iter().cloned())
            .collect(),
    }
}

pub(crate) fn listed(shown: &Shown) -> Vec<Summary> {
    let open = shown.thread.summary();
    let mut listed: Vec<Summary> = shown
        .summaries
        .iter()
        .filter(|summary| summary.id() != open.id())
        .cloned()
        .collect();
    if open.turns() > 0 {
        listed.insert(0, open);
    }
    listed
}
