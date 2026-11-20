use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures_util::Stream;
use rikka_llm_accounts::{ConnectedModel, Llm};
use rikka_tools::{Job, Kind, QuestionEvent, QuestionSet, Selection};

use crate::answer::Answer;
use crate::ask::Ask;
use crate::assistant::Assistant;
use crate::conversation::Conversation;
use crate::error::{Error, Result};
use crate::reply::Reply;
use crate::run::{QueuedAsk, Run, RunOutcome};
use crate::store::Store;
use crate::thread::Thread;

const KEEP_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq)]
pub enum DeskEvent {
    Asked(QuestionEvent),
    JobsMoved,
}

type Notify = Arc<dyn Fn(DeskEvent) + Send + Sync>;

pub struct Desk {
    thread: Arc<Thread>,
    assistant: Option<Assistant>,
    store: Option<Store>,
    run: Run,
    kept_at: Instant,
    questions: Vec<QuestionSet>,
    jobs: Vec<Job>,
    notify: Option<Notify>,
}

pub struct Replies(Pin<Box<dyn Stream<Item = Reply> + Send>>);

impl Stream for Replies {
    type Item = Reply;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Reply>> {
        self.0.as_mut().poll_next(context)
    }
}

impl Desk {
    #[must_use]
    pub fn new(thread: Thread) -> Desk {
        Desk {
            thread: Arc::new(thread),
            assistant: None,
            store: None,
            run: Run::default(),
            kept_at: Instant::now(),
            questions: Vec::new(),
            jobs: Vec::new(),
            notify: None,
        }
    }

    pub fn watch(&mut self, notify: impl Fn(DeskEvent) + Send + Sync + 'static) {
        self.notify = Some(Arc::new(notify));
        self.listen();
    }

    pub fn equip(&mut self, assistant: Assistant) -> Option<Assistant> {
        if let Some(store) = &self.store {
            assistant.keep_files_in(store.history(self.thread.id()), &self.thread);
            assistant.keep_agents_in(store.crew(self.thread.id()));
        }
        let previous = self.assistant.replace(assistant);
        if let (Some(previous), Some(assistant)) = (&previous, &self.assistant) {
            for held in previous.notices().take() {
                assistant.notices().post(held);
            }
        }
        self.questions.clear();
        self.jobs = self
            .assistant
            .as_ref()
            .map_or_else(Vec::new, |assistant| assistant.jobs().snapshot());
        self.listen();
        previous
    }

    pub fn keep_in(&mut self, store: Store) {
        if let Some(assistant) = &self.assistant {
            assistant.keep_files_in(store.history(self.thread.id()), &self.thread);
            assistant.keep_agents_in(store.crew(self.thread.id()));
        }
        store.keep(&self.thread);
        self.store = Some(store);
    }

    pub fn open(&mut self, thread: Thread) -> Arc<Thread> {
        self.run = Run::default();
        if let (Some(store), Some(assistant)) = (&self.store, &self.assistant) {
            assistant.keep_files_in(store.history(thread.id()), &thread);
        }
        std::mem::replace(&mut self.thread, Arc::new(thread))
    }

    #[must_use]
    pub fn thread(&self) -> &Arc<Thread> {
        &self.thread
    }

    pub fn thread_mut(&mut self) -> &mut Thread {
        Arc::make_mut(&mut self.thread)
    }

    pub fn keep(&self) {
        if let Some(store) = &self.store {
            store.keep(&self.thread);
        }
    }

    #[must_use]
    pub fn assistant(&self) -> Option<&Assistant> {
        self.assistant.as_ref()
    }

    #[must_use]
    pub fn answer(&self) -> Option<&Answer> {
        self.run.answer()
    }

    #[must_use]
    pub fn is_answering(&self) -> bool {
        self.run.is_answering()
    }

    #[must_use]
    pub fn is_woken(&self) -> bool {
        !self.run.is_answering()
            && self
                .assistant
                .as_ref()
                .is_some_and(|assistant| assistant.notices().is_waking())
    }

    #[must_use]
    pub fn is_occupied(&self) -> bool {
        self.run.is_answering()
            || self.assistant.as_ref().is_some_and(|assistant| {
                let notices = assistant.notices();
                notices.outstanding() > 0 || notices.is_waking()
            })
    }

    pub fn connect(&mut self, llm: &Llm) -> rikka_llm_accounts::Result<ConnectedModel> {
        self.thread_mut().connect_model(llm)
    }

    pub fn ask(&mut self, hired: ConnectedModel, ask: impl Into<Ask>) -> Result<Replies> {
        let Desk {
            run,
            assistant,
            thread,
            ..
        } = self;
        let assistant = ready(run, assistant.as_ref())?;
        let conversation = Conversation::new(assistant, hired, Arc::make_mut(thread), ask)?;
        Ok(self.begin(conversation))
    }

    pub fn wake(&mut self, hired: ConnectedModel) -> Result<Option<Replies>> {
        let Desk {
            run,
            assistant,
            thread,
            ..
        } = self;
        let assistant = ready(run, assistant.as_ref())?;
        let woken = Conversation::woken(assistant, hired, Arc::make_mut(thread))?;
        Ok(woken.map(|conversation| self.begin(conversation)))
    }

    pub fn compact(&mut self, hired: ConnectedModel, focus: &str) -> Result<Replies> {
        let assistant = ready(&self.run, self.assistant.as_ref())?;
        let conversation = Conversation::compacting(assistant, hired, &self.thread, focus)?;
        Ok(self.begin(conversation))
    }

    pub fn interject(&mut self, ask: Ask) -> std::result::Result<u64, Ask> {
        self.run.interject(ask)
    }

    #[must_use]
    pub fn queued(&self) -> &[QueuedAsk] {
        self.run.queued()
    }

    pub fn withdraw(&mut self, id: u64) -> Option<Ask> {
        self.run.withdraw(id)
    }

    pub fn recall(&mut self) -> Vec<Ask> {
        self.run.recall()
    }

    pub fn stop(&mut self) -> Vec<Ask> {
        if let Some(assistant) = &self.assistant {
            assistant.questions().withdraw();
            assistant.notices().hold();
        }
        self.questions.clear();
        self.run.stop();
        self.run.recall()
    }

    pub fn halt(&mut self) -> Vec<Ask> {
        if let Some(assistant) = &self.assistant {
            let agents = assistant
                .jobs()
                .snapshot()
                .into_iter()
                .filter(|job| job.kind() == Kind::Agent && job.state().is_running());
            for agent in agents {
                assistant.jobs().stop(agent.id());
            }
        }
        self.stop()
    }

    pub fn apply(&mut self, reply: &Reply) -> RunOutcome {
        let Desk { run, thread, .. } = self;
        match run.apply(reply, || Some(Arc::make_mut(thread))) {
            RunOutcome::AnswerUpdated => self.keep_partial(reply),
            RunOutcome::ThreadUpdated => {
                self.keep();
                RunOutcome::ThreadUpdated
            }
            RunOutcome::Finished => {
                if let Some(assistant) = &self.assistant {
                    assistant.questions().withdraw();
                }
                self.questions.clear();
                self.keep();
                RunOutcome::Finished
            }
        }
    }

    pub fn heard(&mut self, event: DeskEvent) {
        match event {
            DeskEvent::Asked(QuestionEvent::Posted(survey)) => self.questions.push(survey),
            DeskEvent::Asked(QuestionEvent::Withdrawn(id)) => {
                self.questions.retain(|survey| survey.id() != id);
            }
            DeskEvent::JobsMoved => {
                if let Some(assistant) = &self.assistant {
                    self.jobs = assistant.jobs().snapshot();
                }
            }
        }
    }

    #[must_use]
    pub fn questions(&self) -> &[QuestionSet] {
        &self.questions
    }

    pub fn answer_question(&mut self, id: u64, selections: Vec<Selection>) {
        if let Some(assistant) = &self.assistant {
            assistant.questions().answer(id, selections);
        }
        self.questions.retain(|survey| survey.id() != id);
    }

    #[must_use]
    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn interrupt_job(&self, id: u64) {
        if let Some(assistant) = &self.assistant {
            assistant.jobs().interrupt(id);
        }
    }

    fn begin(&mut self, conversation: Conversation) -> Replies {
        self.run = Run::from_conversation(&conversation);
        self.kept_at = Instant::now();
        self.keep();
        Replies(Box::pin(conversation.replies()))
    }

    fn keep_partial(&mut self, reply: &Reply) -> RunOutcome {
        let due = match reply {
            Reply::Finished { .. } => true,
            Reply::Delta(_) => self.kept_at.elapsed() >= KEEP_INTERVAL,
            _ => false,
        };
        let Some((text, rounds)) = self.run.finished_answer().filter(|_| due) else {
            return RunOutcome::AnswerUpdated;
        };
        self.thread_mut().answer_partial(text, rounds);
        self.kept_at = Instant::now();
        self.keep();
        RunOutcome::ThreadUpdated
    }

    fn listen(&self) {
        let (Some(notify), Some(assistant)) = (&self.notify, &self.assistant) else {
            return;
        };
        let moved = Arc::clone(notify);
        assistant.jobs().watch(move || moved(DeskEvent::JobsMoved));
        let asked = Arc::clone(notify);
        assistant
            .questions()
            .watch(move |question| asked(DeskEvent::Asked(question)));
    }
}

fn ready<'a>(run: &Run, assistant: Option<&'a Assistant>) -> Result<&'a Assistant> {
    if run.is_answering() {
        return Err(Error::Answering);
    }
    assistant.ok_or(Error::Unequipped)
}

impl std::fmt::Debug for Desk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Desk")
            .field("thread", &self.thread.id())
            .field("answering", &self.run.is_answering())
            .field("questions", &self.questions.len())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Replies {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Replies")
    }
}
