use rikka_sync::Mutex;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_app::prelude::*;
use rikka_chat::{QuestionForm, QuestionQueue};
use rikka_computer::{Computer, Event};
use rikka_computer_use::ComputerUse;
use rikka_llm_accounts::{ConnectedModel, Llm};
use rikka_thread::{
    Answer, Ask, Assistant, Desk, DeskEvent, Job, QueuedAsk, Replies, Reply, RunOutcome, Store,
    Subagent, Thread,
};
use rikka_types::Modifiers;

use crate::project::act::given;
use crate::project::handle::ProjectHandle;
use crate::project::machine::Machine;
use crate::project::machine_input::MachineInput;
use crate::project::state::Project;

pub(crate) struct Runner {
    root: PathBuf,
    built: u64,
    desk: Desk,
    machine: Tracked<Machine>,
    asks: Tracked<QuestionQueue>,
    running: Tracked<bool>,
    waiting: Tracked<Vec<QueuedAsk>>,
    jobs: Tracked<Vec<Job>>,
    asked: Vec<u64>,
    pending_input: VecDeque<MachineInput>,
    driving: bool,
    watching: Arc<Mutex<Option<u64>>>,
    shown: Tracked<Option<u64>>,
    shown_filled: Tracked<u64>,
    handle: ProjectHandle,
}

pub(crate) struct Fitted {
    pub(crate) root: PathBuf,
    pub(crate) built: u64,
    pub(crate) assistant: Assistant,
}

impl Runner {
    pub(crate) fn open(
        thread: Thread,
        fitted: Fitted,
        store: Option<Store>,
        handle: &ProjectHandle,
    ) -> Runner {
        let id = thread.id().to_owned();
        let mut desk = Desk::new(thread);
        if let Some(store) = store {
            desk.keep_in(store);
        }
        let notify = handle.clone();
        desk.watch(move |event| {
            let id = id.clone();
            notify.say(move |project| project.desk_event(&id, event));
        });
        let mut runner = Runner {
            root: PathBuf::new(),
            built: 0,
            desk,
            machine: Tracked::default(),
            asks: Tracked::default(),
            running: Tracked::new(false),
            waiting: Tracked::default(),
            jobs: Tracked::default(),
            asked: Vec::new(),
            pending_input: VecDeque::new(),
            driving: false,
            watching: Arc::default(),
            shown: Tracked::new(None),
            shown_filled: Tracked::new(0),
            handle: handle.clone(),
        };
        let _unequipped = runner.equip(fitted);
        runner
    }

    pub(crate) fn equip(&mut self, fitted: Fitted) -> Option<Assistant> {
        let Fitted {
            root,
            built,
            assistant,
        } = fitted;
        self.root = root;
        self.built = built;
        let previous = self.desk.equip(assistant);
        *self.machine = Machine::default();
        self.pending_input.clear();
        self.mirror_desk();
        self.watch();
        previous
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn built(&self) -> u64 {
        self.built
    }

    pub(crate) fn thread(&self) -> &Thread {
        self.desk.thread()
    }

    pub(crate) fn thread_mut(&mut self) -> &mut Thread {
        self.desk.thread_mut()
    }

    pub(crate) fn keep(&self) {
        self.desk.keep();
    }

    pub(crate) fn keep_in(&mut self, store: Store) {
        self.desk.keep_in(store);
    }

    pub(crate) fn machine(&self) -> &Machine {
        &self.machine
    }

    pub(crate) fn is_running(&self) -> bool {
        *self.running
    }

    pub(crate) fn arriving(&self) -> Option<&Answer> {
        self.desk.answer()
    }

    pub(crate) fn watching(&self) -> Option<u64> {
        *self.shown
    }

    pub(crate) fn watch_agent(&mut self, job: u64) {
        self.show(Some(job));
    }

    pub(crate) fn watch_main(&mut self) {
        self.show(None);
    }

    fn show(&mut self, job: Option<u64>) {
        *self.watching.lock() = job;
        self.shown.set(job);
    }

    pub(crate) fn is_shown_running(&self) -> bool {
        match self.watching() {
            Some(job) => self
                .jobs
                .iter()
                .any(|shown| shown.id() == job && shown.state().is_running()),
            None => self.is_running(),
        }
    }

    pub(crate) fn shown_filled(&self) -> u64 {
        *self.shown_filled
    }

    pub(crate) fn mirror(&mut self, hand: Option<&Subagent>) {
        self.shown_filled
            .set(hand.map_or(0, |hand| hand.thread().context_tokens()));
    }

    #[must_use]
    pub(crate) fn hand(&self) -> Option<Subagent> {
        let job = self.watching()?;
        self.desk.assistant()?.subagents()?.get(job)
    }

    pub(crate) fn asking(&self) -> Option<&QuestionForm> {
        self.asks.open()
    }

    pub(crate) fn heard(&mut self, event: DeskEvent) {
        self.desk.heard(event);
        self.mirror_desk();
    }

    pub(crate) fn connect(&mut self, llm: &Llm) -> rikka_llm_accounts::Result<ConnectedModel> {
        self.desk.connect(llm)
    }

    pub(crate) fn ask(&mut self, hired: ConnectedModel, ask: Ask) -> rikka_thread::Result<()> {
        let replies = self.desk.ask(hired, ask)?;
        self.say(replies);
        Ok(())
    }

    pub(crate) fn wake(&mut self, hired: ConnectedModel) -> rikka_thread::Result<bool> {
        let Some(replies) = self.desk.wake(hired)? else {
            return Ok(false);
        };
        self.say(replies);
        Ok(true)
    }

    pub(crate) fn compact(
        &mut self,
        hired: ConnectedModel,
        focus: &str,
    ) -> rikka_thread::Result<()> {
        let replies = self.desk.compact(hired, focus)?;
        self.say(replies);
        Ok(())
    }

    fn say(&mut self, replies: Replies) {
        self.running.set(true);
        self.mirror_desk();
        let thread = self.desk.thread().id().to_owned();
        stream(replies, move |project: &mut Project, reply: Reply| {
            project.replied(&thread, reply)
        });
    }

    pub(crate) fn apply(&mut self, reply: &Reply) -> RunOutcome {
        let applied = self.desk.apply(reply);
        match applied {
            RunOutcome::Finished => {
                self.running.set(false);
                self.mirror_desk();
            }
            RunOutcome::ThreadUpdated if matches!(reply, Reply::Interjected { .. }) => {
                self.mirror_desk();
            }
            RunOutcome::ThreadUpdated | RunOutcome::AnswerUpdated => {}
        }
        applied
    }

    pub(crate) fn waiting(&self) -> &[QueuedAsk] {
        &self.waiting
    }

    pub(crate) fn interject(&mut self, ask: Ask) -> Result<u64, Ask> {
        let id = self.desk.interject(ask)?;
        self.mirror_desk();
        Ok(id)
    }

    pub(crate) fn unqueue(&mut self, id: u64) -> Option<Ask> {
        let ask = self.desk.withdraw(id);
        self.mirror_desk();
        ask
    }

    pub(crate) fn unheard(&mut self) -> Vec<Ask> {
        let unheard = self.desk.recall();
        self.mirror_desk();
        unheard
    }

    pub(crate) fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub(crate) fn stop_job(&self, id: u64) {
        self.desk.interrupt_job(id);
    }

    pub(crate) fn is_woken(&self) -> bool {
        self.desk.is_woken()
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.desk.is_answering()
            || self
                .desk
                .assistant()
                .is_some_and(|assistant| assistant.notices().is_waking())
            || self
                .jobs
                .iter()
                .any(|job| job.kind() == rikka_thread::Kind::Agent && job.state().is_running())
    }

    pub(crate) fn is_occupied(&self) -> bool {
        self.desk.is_occupied()
    }

    pub(crate) fn stop(&mut self) -> Vec<Ask> {
        let unheard = self.desk.stop();
        self.mirror_desk();
        unheard
    }

    pub(crate) fn halt(&mut self) {
        let _unheard = self.desk.halt();
        self.mirror_desk();
    }

    pub(crate) fn into_desk(self) -> Desk {
        self.desk
    }

    pub(crate) fn choose(&mut self, option: usize) {
        if let Some(asking) = self.asks.open_mut() {
            asking.choose(option);
        }
    }

    pub(crate) fn wrote(&mut self) {
        if let Some(asking) = self.asks.open_mut() {
            asking.wrote();
        }
    }

    pub(crate) fn back(&mut self) {
        if let Some(asking) = self.asks.open_mut() {
            asking.back();
        }
    }

    pub(crate) fn ahead(&mut self) {
        if let Some((id, picked)) = self.asks.ahead() {
            self.desk.answer_question(id, picked);
        }
    }

    pub(crate) fn queue_input(&mut self, input: MachineInput) {
        self.pending_input.push_back(input);
        self.driving();
    }

    fn mirror_desk(&mut self) {
        self.running.set(self.desk.is_answering());
        self.waiting.set(self.desk.queued().to_vec());
        let jobs = self.desk.jobs();
        let moved = jobs.len() != self.jobs.len()
            || std::iter::zip(jobs, self.jobs.iter()).any(|(job, shown)| {
                job.id() != shown.id()
                    || job.state() != shown.state()
                    || job.label() != shown.label()
            });
        if moved {
            *self.jobs = jobs.to_vec();
        }
        let asked: Vec<u64> = self
            .desk
            .questions()
            .iter()
            .map(|survey| survey.id())
            .collect();
        if asked != self.asked {
            self.asks.track(self.desk.questions());
            self.asked = asked;
        }
    }

    fn computer(&self) -> Option<&Computer> {
        self.desk
            .assistant()?
            .machine()
            .and_then(ComputerUse::computer_of)
    }

    fn framed(&mut self) {
        match self.machine.is_lit() {
            true => skip_view(),
            false => self.machine.light(),
        }
    }

    fn drove(&mut self) {
        self.driving = false;
        self.driving();
    }

    fn driving(&mut self) {
        if self.driving || self.pending_input.is_empty() {
            skip_view();
            return;
        }

        let Some(computer) = self.computer().cloned() else {
            self.pending_input.clear();
            skip_view();
            return;
        };
        self.driving = true;
        let pending_input: Vec<MachineInput> = self.pending_input.drain(..).collect();
        let thread = self.desk.thread().id().to_owned();
        spawn_blocking(
            move || {
                for input in &pending_input {
                    apply_input(&computer, input);
                }
            },
            given(move |project, (): ()| project.with_runner(&thread, Runner::drove)),
        );
    }

    fn watch(&self) {
        let Some(assistant) = self.desk.assistant() else {
            return;
        };
        let thread = self.desk.thread().id().to_owned();
        if let Some(crew) = assistant.subagents() {
            let notify = self.handle.clone();
            let thread = thread.clone();
            let watching = Arc::clone(&self.watching);
            crew.watch(move |job| {
                if *watching.lock() != Some(job) {
                    return;
                }
                let thread = thread.clone();
                notify.reach(move |project| project.agent_said(&thread));
            });
        }

        let notify = self.handle.clone();
        let feed = self.machine.feed();
        let Some(computer) = self.computer() else {
            return;
        };
        computer.watch(move |screen| {
            let thread = thread.clone();
            match screen {
                Event::Frame(frame) => {
                    feed.show(frame.width(), frame.height(), frame.into_pixels());
                    notify.say(move |project| project.with_runner(&thread, Runner::framed));
                }
                Event::Status(standing) => notify.say(move |project| {
                    project.with_runner(&thread, |runner| runner.machine.stands(standing));
                }),
                Event::Disconnected => {
                    notify.say(move |project| {
                        project.with_runner(&thread, |runner| runner.machine.lost())
                    });
                }
            }
        });
    }
}

fn apply_input(computer: &Computer, input: &MachineInput) {
    let done = match input {
        MachineInput::PointerMove(x, y) => computer.move_to(*x, *y),
        MachineInput::PointerPress {
            button,
            at,
            modifiers,
        } => computer
            .hold_modifiers(*modifiers)
            .and_then(|()| computer.press(*button, Some(*at))),
        MachineInput::PointerRelease { button, at } => computer
            .release(*button, Some(*at))
            .and_then(|()| computer.hold_modifiers(Modifiers::NONE)),
        MachineInput::Scroll { at, across, down } => {
            computer.scroll(*across, *down, Modifiers::NONE, Some(*at))
        }
        MachineInput::Key(chord) => computer.key(*chord),
        MachineInput::Text(text) => computer.type_text(text),
    };
    if let Err(error) = done {
        tracing::debug!(%error, "computer action failed");
    }
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner")
            .field("root", &self.root)
            .field("running", &self.is_running())
            .finish_non_exhaustive()
    }
}
