use rikka_sync::Mutex;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rikka_agent::{Notice, Notices, PendingNotice};
use rikka_llm::{Effort, Usage};
use rikka_prompt::escape_attribute;
use rikka_types::CancelToken;

use crate::output_buffer::OutputBuffer;
use crate::shell::{Ended, Running};

type Watcher = Arc<dyn Fn() + Send + Sync>;

const KEPT: usize = 64;
const GIST: usize = 200;

const VANISHED: &str = "it ended without reporting";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    Agent,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Command => "command",
            Kind::Agent => "agent",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Exited(i32),
    Signalled,
    Stopped,
    Interrupted,
    Closed,
    Reported,
    Failed(String),
}

impl State {
    fn label(&self) -> String {
        match self {
            State::Running => "running".to_owned(),
            State::Exited(code) => format!("exited {code}"),
            State::Signalled => "killed by a signal".to_owned(),
            State::Stopped => "stopped".to_owned(),
            State::Interrupted => "stopped by the person".to_owned(),
            State::Closed => "stopped when the app closed".to_owned(),
            State::Reported => "reported".to_owned(),
            State::Failed(reason) => format!("failed: {reason}"),
        }
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        *self == State::Running
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct On {
    model: String,
    effort: Option<Effort>,
}

impl On {
    #[must_use]
    pub fn new(model: impl Into<String>, effort: Option<Effort>) -> On {
        On {
            model: model.into(),
            effort,
        }
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub fn effort(&self) -> Option<Effort> {
        self.effort
    }
}

struct OnAttributes<'a>(Option<&'a On>);

impl fmt::Display for OnAttributes<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(on) => on.fmt(out),
            None => Ok(()),
        }
    }
}

impl fmt::Display for On {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, " model=\"{}\"", escape_attribute(&self.model))?;
        if let Some(effort) = self.effort.and_then(Effort::name) {
            write!(out, " effort=\"{effort}\"")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    id: u64,
    kind: Kind,
    label: String,
    on: Option<On>,
    state: State,
}

impl Job {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn on(&self) -> Option<&On> {
        self.on.as_ref()
    }

    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }
}

#[derive(Clone, Debug)]
pub struct Jobs {
    inner: Arc<Inner>,
}

struct Inner {
    next_id: AtomicU64,
    jobs: Mutex<BTreeMap<u64, JobEntry>>,
    notices: Notices,
    watcher: Mutex<Option<Watcher>>,
    retain: usize,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Jobs")
            .field("kept", &self.jobs.lock().len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct JobEntry {
    kind: Kind,
    label: String,
    on: Option<On>,
    started: Instant,
    trail: Arc<Mutex<OutputBuffer>>,
    log: Option<String>,
    stop: Stop,
    state: State,
}

#[derive(Debug)]
enum Stop {
    Command(Running),
    Agent(CancelToken),
}

impl Jobs {
    #[must_use]
    pub fn new(notices: Notices, retain: usize) -> Jobs {
        Jobs {
            inner: Arc::new(Inner {
                next_id: AtomicU64::new(0),
                jobs: Mutex::new(BTreeMap::new()),
                notices,
                watcher: Mutex::new(None),
                retain: retain.max(1),
            }),
        }
    }

    pub fn watch(&self, watcher: impl Fn() + Send + Sync + 'static) {
        *self.inner.watcher.lock() = Some(Arc::new(watcher));
    }

    #[must_use]
    pub fn notices(&self) -> &Notices {
        &self.inner.notices
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<Job> {
        self.inner
            .jobs
            .lock()
            .iter()
            .map(|(id, job)| job.snapshot(*id))
            .collect()
    }

    #[must_use]
    pub fn summary(&self) -> Option<String> {
        let jobs = self.inner.jobs.lock();
        if jobs.is_empty() {
            return None;
        }

        let listing = fmt::from_fn(|out| {
            out.write_str("<jobs>")?;
            for (id, job) in jobs.iter() {
                write!(
                    out,
                    "\n<job id=\"{id}\" kind=\"{}\"{} state=\"{}\" for=\"{}\" unread=\"{} bytes\">\n{}\n</job>",
                    job.kind.as_str(),
                    OnAttributes(job.on.as_ref()),
                    escape_attribute(&job.state.label()),
                    format_elapsed(job.started.elapsed()),
                    job.unread(),
                    gist(&job.label)
                )?;
            }
            out.write_str("\n</jobs>")
        });
        Some(listing.to_string())
    }

    #[must_use]
    pub fn running(&self) -> usize {
        self.inner
            .jobs
            .lock()
            .values()
            .filter(|job| job.state.is_running())
            .count()
    }

    pub fn stop(&self, id: u64) -> bool {
        self.halt(id, State::Stopped)
    }

    pub fn interrupt(&self, id: u64) -> bool {
        self.halt(id, State::Interrupted)
    }

    #[must_use]
    pub fn state(&self, id: u64) -> Option<State> {
        self.inner.jobs.lock().get(&id).map(|job| job.state.clone())
    }

    fn halt(&self, id: u64, state: State) -> bool {
        let halted = {
            let mut jobs = self.inner.jobs.lock();
            let Some(job) = jobs.get_mut(&id) else {
                return false;
            };
            if !job.state.is_running() {
                return false;
            }
            job.state = state;
            match &job.stop {
                Stop::Command(running) => running.kill(),
                Stop::Agent(cancel) => cancel.cancel(),
            }
            true
        };

        self.changed();
        halted
    }

    pub fn agent(&self, label: &str, on: On, cancel: CancelToken) -> Ticket {
        let id = self.hold(JobEntry {
            kind: Kind::Agent,
            label: label.to_owned(),
            on: Some(on),
            started: Instant::now(),
            trail: Arc::new(Mutex::new(OutputBuffer::new(self.inner.retain))),
            log: None,
            stop: Stop::Agent(cancel),
            state: State::Running,
        });

        self.ticket(id)
    }

    pub fn restart(&self, id: u64, on: On, cancel: CancelToken) -> Option<Ticket> {
        {
            let mut jobs = self.inner.jobs.lock();
            let job = jobs.get_mut(&id)?;
            if job.kind != Kind::Agent || job.state.is_running() {
                return None;
            }
            job.state = State::Running;
            job.started = Instant::now();
            job.stop = Stop::Agent(cancel);
            job.on = Some(on);
        }

        self.changed();
        Some(self.ticket(id))
    }

    pub fn restore_agent(&self, id: u64, label: &str, on: On, state: State) -> Writer {
        {
            let mut jobs = self.inner.jobs.lock();
            self.inner.next_id.fetch_max(id, Ordering::Relaxed);
            jobs.insert(
                id,
                JobEntry {
                    kind: Kind::Agent,
                    label: label.to_owned(),
                    on: Some(on),
                    started: Instant::now(),
                    trail: Arc::new(Mutex::new(OutputBuffer::new(self.inner.retain))),
                    log: None,
                    stop: Stop::Agent(CancelToken::new()),
                    state,
                },
            );
            evict_oldest_ended(&mut jobs);
        }

        self.changed();
        Writer {
            jobs: self.clone(),
            id,
        }
    }

    pub fn remind(&self, id: u64) {
        let notice = self
            .inner
            .jobs
            .lock()
            .get(&id)
            .filter(|job| !job.state.is_running())
            .map(|job| job.notice(id));
        if let Some(notice) = notice {
            self.inner.notices.post(Notice::new(notice).held());
            self.changed();
        }
    }

    #[must_use]
    pub fn contains(&self, id: u64) -> bool {
        self.inner.jobs.lock().contains_key(&id)
    }

    fn wrote(&self, id: u64, text: &str) {
        let jobs = self.inner.jobs.lock();
        if let Some(job) = jobs.get(&id) {
            job.trail.lock().push(text.as_bytes());
        }
    }

    fn ticket(&self, id: u64) -> Ticket {
        Ticket {
            jobs: self.clone(),
            id,
            pending: Some(self.inner.notices.pending()),
        }
    }

    pub(crate) fn command(&self, command: &str, running: Running, log: Option<String>) -> u64 {
        let id = self.hold(JobEntry {
            kind: Kind::Command,
            label: command.to_owned(),
            on: None,
            started: Instant::now(),
            trail: Arc::clone(running.trail()),
            log,
            stop: Stop::Command(running.clone()),
            state: State::Running,
        });

        let pending = self.inner.notices.pending();
        let jobs = self.clone();
        tokio::spawn(async move {
            let ended = running.wait_exit().await;
            jobs.ended(
                id,
                match ended {
                    Ended::Exited(code) => State::Exited(code),
                    Ended::Signalled => State::Signalled,
                },
                pending,
                Usage::default(),
            );
        });

        id
    }

    pub(crate) fn read(&self, id: u64, room: usize) -> std::result::Result<String, String> {
        let jobs = self.inner.jobs.lock();
        let job = jobs.get(&id).ok_or_else(|| unknown(id))?;
        let state = job.state.clone();
        if let Some(log) = job.log.clone() {
            drop(jobs);
            return Ok(format!(
                "({}) its whole output is in {log}; read what you need out of that file.",
                state.label()
            ));
        }
        let output = job.trail.lock().read_new(room).trim_end().to_owned();
        drop(jobs);

        Ok(match (output.trim().is_empty(), state.is_running()) {
            (true, true) => "(nothing new; it is still running)".to_owned(),
            (true, false) => format!("({})", state.label()),
            (false, true) => output,
            (false, false) => format!("({})\n{output}", state.label()),
        })
    }

    pub(crate) fn listing(&self) -> String {
        self.summary()
            .unwrap_or_else(|| "no job has been started".to_owned())
    }

    pub(crate) fn label(&self, id: u64) -> Option<String> {
        self.inner.jobs.lock().get(&id).map(|job| job.label.clone())
    }

    fn hold(&self, job: JobEntry) -> u64 {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut jobs = self.inner.jobs.lock();
        jobs.insert(id, job);
        evict_oldest_ended(&mut jobs);
        drop(jobs);

        self.changed();
        id
    }

    fn ended(&self, id: u64, state: State, pending: PendingNotice, usage: Usage) {
        let notice = {
            let mut jobs = self.inner.jobs.lock();
            let Some(job) = jobs.get_mut(&id) else {
                return;
            };
            let notice = match job.state {
                State::Stopped => None,
                State::Interrupted => Some(Notice::new(job.notice(id)).held()),
                _ => {
                    job.state = state;
                    Some(Notice::new(job.notice(id)))
                }
            };
            evict_oldest_ended(&mut jobs);
            notice
        };

        match notice {
            Some(notice) => pending.post(notice.with_usage(usage)),
            None => drop(pending),
        }
        self.changed();
    }

    fn abandoned(&self, id: u64, reason: &str, pending: PendingNotice) {
        if let Some(job) = self.inner.jobs.lock().get_mut(&id) {
            job.state = State::Failed(reason.to_owned());
        }
        drop(pending);
        self.changed();
    }

    fn changed(&self) {
        let watcher = self.inner.watcher.lock().clone();
        if let Some(watcher) = watcher {
            watcher();
        }
    }
}

impl JobEntry {
    fn snapshot(&self, id: u64) -> Job {
        Job {
            id,
            kind: self.kind,
            label: self.label.clone(),
            on: self.on.clone(),
            state: self.state.clone(),
        }
    }

    fn unread(&self) -> usize {
        match self.log {
            Some(_) => 0,
            None => self.trail.lock().unread_len(),
        }
    }

    fn notice(&self, id: u64) -> String {
        let unread = self.unread();
        let read = match &self.log {
            Some(log) => format!("Its whole output is in {log}."),
            None => format!(
                "Read what it left with the Jobs tool: {{ \"action\": \"output\", \"id\": {id} }}."
            ),
        };

        format!(
            "<job_ended id=\"{id}\" kind=\"{}\"{} state=\"{}\" for=\"{}\" unread=\"{unread} bytes\">\n{}\n{read}\n</job_ended>",
            self.kind.as_str(),
            OnAttributes(self.on.as_ref()),
            escape_attribute(&self.state.label()),
            format_elapsed(self.started.elapsed()),
            self.label.trim(),
        )
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        for job in self.jobs.lock().values() {
            match &job.stop {
                Stop::Command(running) => running.kill(),
                Stop::Agent(cancel) => cancel.cancel(),
            }
        }
    }
}

#[must_use = "an agent that never reports through its ticket is reported as vanished"]
#[derive(Debug)]
pub struct Ticket {
    jobs: Jobs,
    id: u64,
    pending: Option<PendingNotice>,
}

impl Ticket {
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub fn writer(&self) -> Writer {
        Writer {
            jobs: self.jobs.clone(),
            id: self.id,
        }
    }

    pub fn append_output(&self, text: &str) {
        self.writer().append_output(text);
    }

    pub fn finish(mut self, text: &str, trouble: Option<&str>, usage: Usage) {
        self.append_output(text);
        let Some(pending) = self.pending.take() else {
            return;
        };

        let state = match trouble {
            Some(trouble) => State::Failed(trouble.to_owned()),
            None => State::Reported,
        };
        self.jobs.ended(self.id, state, pending, usage);
    }

    pub fn abandon(mut self, reason: &str) {
        if let Some(pending) = self.pending.take() {
            self.jobs.abandoned(self.id, reason, pending);
        }
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };

        self.jobs.ended(
            self.id,
            State::Failed(VANISHED.to_owned()),
            pending,
            Usage::default(),
        );
    }
}

#[derive(Clone, Debug)]
pub struct Writer {
    jobs: Jobs,
    id: u64,
}

impl Writer {
    pub fn append_output(&self, text: &str) {
        self.jobs.wrote(self.id, text);
    }
}

pub(crate) fn unknown(id: u64) -> String {
    format!("there is no job {id}")
}

fn evict_oldest_ended(jobs: &mut BTreeMap<u64, JobEntry>) {
    let ended: Vec<u64> = jobs
        .iter()
        .filter(|(_, job)| !job.state.is_running())
        .map(|(id, _)| *id)
        .collect();

    for id in ended.into_iter().rev().skip(KEPT) {
        jobs.remove(&id);
    }
}

fn gist(label: &str) -> Cow<'_, str> {
    rikka_format::truncate(label.trim(), GIST)
}

fn format_elapsed(ran: Duration) -> String {
    let seconds = ran.as_secs();
    match seconds {
        0..60 => format!("{seconds}s"),
        _ => format!("{}m {}s", seconds / 60, seconds % 60),
    }
}
