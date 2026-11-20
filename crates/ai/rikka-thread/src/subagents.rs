use rikka_sync::Mutex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rikka_agent::{
    Agent, Budget, Compaction, CompactionPrompts, Error as AgentError, Event, Limit, Notice,
    Outcome,
};
use rikka_llm::{Effort, HostedPart, Message, Trouble, Usage};
use rikka_llm_accounts::ConnectedModel;
use rikka_tools::{Jobs, On, Ticket, Toolbox, Writer};
use rikka_types::CancelToken;

use crate::conversation::{joined, reply_for};
use crate::fitting::{Fitting, applied};
use crate::reply::Reply;
use crate::run_context::RunContext;
use crate::store::{Crew, Member, Standing};
use crate::subagent::{Holding, Subagent, SubagentTranscript};
use crate::thread::Thread;

#[derive(Debug)]
pub(crate) struct TaskRequest {
    pub(crate) label: String,
    pub(crate) task: String,
    pub(crate) job: Option<u64>,
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<Effort>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DispatchKind {
    Started,
    Sent,
    Resumed,
}

impl DispatchKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            DispatchKind::Started => "started",
            DispatchKind::Sent => "sent",
            DispatchKind::Resumed => "resumed",
        }
    }
}

#[derive(Debug)]
pub(crate) struct Started {
    pub(crate) label: String,
    pub(crate) job: u64,
    pub(crate) model: String,
    pub(crate) kind: DispatchKind,
}

#[derive(Debug, Default)]
pub(crate) struct DispatchReport {
    pub(crate) started: Vec<Started>,
    pub(crate) errors: Vec<String>,
}

type Following = Arc<dyn Fn(u64) + Send + Sync>;

#[derive(Clone, Debug)]
pub struct Subagents {
    inner: Arc<Inner>,
}

struct Inner {
    toolbox: Arc<Toolbox>,
    briefing: Vec<String>,
    folding: CompactionPrompts,
    budget: Budget,
    run_context: Mutex<Option<RunContext>>,
    workers: Mutex<Workers>,
    following: Mutex<Option<Following>>,
    crew: Mutex<Option<Crew>>,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Subagents")
            .field("workers", &self.workers.lock().len())
            .finish_non_exhaustive()
    }
}

type Workers = BTreeMap<u64, TrackedWorker>;

#[derive(Debug)]
struct TrackedWorker {
    label: String,
    on: On,
    hired: Option<ConnectedModel>,
    toolbox: Option<Arc<Toolbox>>,
    writer: Writer,
    running: bool,
    kept: bool,
    report: String,
    history: Option<Vec<Message>>,
    transcript: Option<SubagentTranscript>,
}

impl TrackedWorker {
    fn model(&self) -> &str {
        self.hired
            .as_ref()
            .map_or_else(|| self.on.model(), |hired| hired.model().name())
    }

    fn member(&self, job: u64, standing: Standing) -> Member {
        Member {
            job,
            label: self.label.clone(),
            model: self.on.model().to_owned(),
            effort: self.on.effort(),
            standing,
            report: self.report.clone(),
        }
    }
}

enum Plan {
    Fresh {
        label: String,
        task: String,
        hired: ConnectedModel,
    },
    Send {
        job: u64,
        task: String,
    },
    Resume {
        job: u64,
        task: String,
        hired: ConnectedModel,
    },
    Refused(String),
}

impl Plan {
    fn job(&self) -> Option<u64> {
        match self {
            Plan::Fresh { .. } | Plan::Refused(_) => None,
            Plan::Send { job, .. } | Plan::Resume { job, .. } => Some(*job),
        }
    }
}

type MessageResult<T> = std::result::Result<T, String>;

impl Subagents {
    pub(crate) fn new(
        toolbox: Arc<Toolbox>,
        briefing: Vec<String>,
        folding: CompactionPrompts,
        budget: Budget,
    ) -> Subagents {
        Subagents {
            inner: Arc::new(Inner {
                toolbox,
                briefing,
                folding,
                budget,
                run_context: Mutex::new(None),
                workers: Mutex::new(Workers::new()),
                following: Mutex::new(None),
                crew: Mutex::new(None),
            }),
        }
    }

    pub fn watch(&self, following: impl Fn(u64) + Send + Sync + 'static) {
        *self.inner.following.lock() = Some(Arc::new(following));
    }

    #[must_use]
    pub fn get(&self, job: u64) -> Option<Subagent> {
        let retained = self.inner.toolbox.jobs().contains(job);
        let mut workers = self.inner.workers.lock();
        if !retained {
            workers.remove(&job);
            return None;
        }
        let worker = workers.get(&job)?;
        let model = worker
            .hired
            .as_ref()
            .and_then(ConnectedModel::info)
            .cloned();
        worker
            .transcript
            .as_ref()
            .map(|transcript| transcript.snapshot(model))
    }

    pub(crate) fn keep_in(&self, crew: Crew) {
        let jobs = self.inner.toolbox.jobs();
        let recalled = crew.recalled();
        let mut workers = self.inner.workers.lock();
        for member in recalled {
            if workers.contains_key(&member.job) || jobs.contains(member.job) {
                tracing::debug!(
                    job = member.job,
                    "agent job is taken, so it stays unrestored"
                );
                continue;
            }
            let on = On::new(member.model.clone(), member.effort);
            let writer = jobs.restore_agent(
                member.job,
                &member.label,
                on.clone(),
                member.standing.state(),
            );
            writer.append_output(&member.report);
            if member.standing == Standing::Running {
                jobs.remind(member.job);
                crew.keep(Member {
                    standing: Standing::Closed,
                    ..member.clone()
                });
            }
            workers.insert(
                member.job,
                TrackedWorker {
                    label: member.label,
                    on,
                    hired: None,
                    toolbox: None,
                    writer,
                    running: false,
                    kept: true,
                    report: member.report,
                    history: None,
                    transcript: None,
                },
            );
        }
        *self.inner.crew.lock() = Some(crew);
    }

    pub(crate) fn set_run_context(&self, run_context: RunContext) {
        *self.inner.run_context.lock() = Some(run_context);
    }

    pub(crate) fn dispatch(
        &self,
        task_requests: Vec<TaskRequest>,
    ) -> MessageResult<DispatchReport> {
        let Some(run_context) = self.inner.run_context.lock().clone() else {
            return Err("there is no conversation for an agent to answer in".to_owned());
        };

        let jobs = self.inner.toolbox.jobs().clone();
        let mut workers = self.inner.workers.lock();
        workers.retain(|job, _| jobs.contains(*job));

        let plans: Vec<Plan> = task_requests
            .into_iter()
            .map(|task_request| planned(&run_context, &jobs, &workers, task_request))
            .collect();
        named_once(&plans)?;

        let mut report = DispatchReport::default();
        for plan in plans {
            let kind = match plan {
                Plan::Fresh { label, task, hired } => {
                    self.start(&run_context, &jobs, &mut workers, label, task, hired)
                }
                Plan::Send { job, task } => Ok(sent(&workers, job, &task)),
                Plan::Resume { job, task, hired } => {
                    self.again(&run_context, &jobs, &mut workers, job, task, hired)
                }
                Plan::Refused(trouble) => Err(trouble),
            };
            match kind {
                Ok(started) => report.started.push(started),
                Err(trouble) => report.errors.push(trouble),
            }
        }
        drop(workers);
        for started in &report.started {
            self.wake(started.job);
        }

        match report.started.is_empty() {
            true => Err(report.errors.join("\n")),
            false => Ok(report),
        }
    }

    fn start(
        &self,
        run_context: &RunContext,
        jobs: &Jobs,
        workers: &mut Workers,
        label: String,
        task: String,
        hired: ConnectedModel,
    ) -> MessageResult<Started> {
        let toolbox = Arc::new(
            self.inner
                .toolbox
                .for_subagent()
                .map_err(|error| error.to_string())?,
        );
        let own = CancelToken::new();
        let on = running_on(&hired);
        let ticket = jobs.agent(&label, on.clone(), own.clone());
        let job = ticket.id();
        let instructions = self.briefing(&hired);
        let agent = match self.agent(run_context, &ticket, own, &toolbox, &hired, &instructions) {
            Ok(agent) => agent,
            Err(error) => {
                ticket.abandon(&error);
                return Err(error);
            }
        };

        let kept = self.crew().is_some();
        let mut transcript = self.fresh_transcript(job);
        transcript.push_task(&task, Some(instructions));
        let model = hired.label();
        let worker = TrackedWorker {
            label: label.clone(),
            on,
            hired: Some(hired),
            toolbox: Some(toolbox),
            writer: ticket.writer(),
            running: true,
            kept,
            report: String::new(),
            history: None,
            transcript: Some(transcript),
        };
        self.remember(job, &worker, Standing::Running);
        workers.insert(job, worker);
        self.run(ticket, agent, task, Vec::new());

        Ok(Started {
            label,
            job,
            model,
            kind: DispatchKind::Started,
        })
    }

    fn again(
        &self,
        run_context: &RunContext,
        jobs: &Jobs,
        workers: &mut Workers,
        job: u64,
        task: String,
        hired: ConnectedModel,
    ) -> MessageResult<Started> {
        let worker = workers.get_mut(&job).expect("planned job should exist");
        let toolbox = match &worker.toolbox {
            Some(toolbox) => Arc::clone(toolbox),
            None => Arc::new(
                self.inner
                    .toolbox
                    .for_subagent()
                    .map_err(|error| error.to_string())?,
            ),
        };
        let mut transcript = match worker.transcript.take() {
            Some(transcript) => transcript,
            None => self.recalled_transcript(job)?,
        };
        let instructions = transcript
            .instructions()
            .unwrap_or_else(|| self.briefing(&hired));

        let own = CancelToken::new();
        let on = running_on(&hired);
        let Some(ticket) = jobs.restart(job, on.clone(), own.clone()) else {
            worker.transcript = Some(transcript);
            return Err(format!(
                "job {job} is no longer kept, so it cannot be run again"
            ));
        };
        let agent = match self.agent(run_context, &ticket, own, &toolbox, &hired, &instructions) {
            Ok(agent) => agent,
            Err(error) => {
                ticket.abandon(&error);
                worker.transcript = Some(transcript);
                self.remember_ended(jobs, job, worker);
                return Err(error);
            }
        };

        let history = worker
            .history
            .take()
            .unwrap_or_else(|| transcript.thread().transcript(hired.model().name()));
        let label = worker.label.clone();
        let model = hired.label();
        ticket.append_output(&carried(DispatchKind::Resumed, &task));
        transcript.push_task(&task, None);
        worker.on = on;
        worker.hired = Some(hired);
        worker.toolbox = Some(toolbox);
        worker.writer = ticket.writer();
        worker.running = true;
        worker.transcript = Some(transcript);
        self.remember(job, worker, Standing::Running);
        self.run(ticket, agent, task, history);

        Ok(Started {
            label,
            job,
            model,
            kind: DispatchKind::Resumed,
        })
    }

    fn run(&self, ticket: Ticket, agent: Agent, task: String, history: Vec<Message>) {
        let subagents = self.clone();
        let job = ticket.id();
        tokio::spawn(async move {
            let mut session = agent.resume(history);
            let mut ticket = ticket;
            let mut prompt = task;
            loop {
                let outcome = session.send(prompt).await;
                let history = session.history().to_vec();
                match subagents.settle(job, ticket, history, outcome) {
                    Some((pending, more)) => (ticket, prompt) = (pending, more),
                    None => return,
                }
            }
        });
    }

    fn settle(
        &self,
        job: u64,
        ticket: Ticket,
        history: Vec<Message>,
        outcome: rikka_agent::Result<Outcome>,
    ) -> Option<(Ticket, String)> {
        let jobs = self.inner.toolbox.jobs();
        let mut workers = self.inner.workers.lock();
        let Some(worker) = workers.get_mut(&job) else {
            reported(ticket, outcome);
            return None;
        };

        if outcome.is_ok() {
            let arrived = worker
                .toolbox
                .as_ref()
                .map(|toolbox| toolbox.jobs().notices().take())
                .unwrap_or_default();
            if !arrived.is_empty() {
                let more = joined(&arrived);
                if let Some(transcript) = worker.transcript.as_mut() {
                    transcript.push_task(&more, None);
                }
                self.remember_transcript(job, worker);
                drop(workers);
                self.wake(job);
                return Some((ticket, more));
            }
        }

        worker.history = Some(history);
        worker.running = false;
        let end = ended(&outcome);
        let model = worker.model().to_owned();
        if let Some(transcript) = worker.transcript.as_mut() {
            transcript.apply(&end, &model);
        }
        let report = reported(ticket, outcome);
        if !report.is_empty() {
            worker.report = report;
        }
        self.remember_ended(jobs, job, worker);
        drop(workers);
        self.wake(job);
        None
    }

    fn crew(&self) -> Option<Crew> {
        self.inner.crew.lock().clone()
    }

    fn fresh_transcript(&self, job: u64) -> SubagentTranscript {
        match self.crew() {
            Some(crew) => {
                SubagentTranscript::new(Thread::with_id(crew.transcript_id(job)), Holding::Whole)
            }
            None => SubagentTranscript::new(Thread::new(), Holding::Bounded),
        }
    }

    fn recalled_transcript(&self, job: u64) -> MessageResult<SubagentTranscript> {
        let recalled = self
            .crew()
            .map(|crew| crew.transcript(job))
            .transpose()
            .map_err(|error| format!("job {job} cannot be read back: {error}"))?
            .flatten()
            .ok_or_else(|| format!("job {job} kept nothing to carry on from"))?;
        Ok(SubagentTranscript::new(recalled, Holding::Whole))
    }

    fn remember(&self, job: u64, worker: &TrackedWorker, standing: Standing) {
        let Some(crew) = self.crew().filter(|_| worker.kept) else {
            return;
        };
        crew.keep(worker.member(job, standing));
        if let Some(transcript) = &worker.transcript {
            crew.keep_transcript(job, transcript.thread());
        }
    }

    fn remember_ended(&self, jobs: &Jobs, job: u64, worker: &TrackedWorker) {
        let Some(state) = jobs.state(job) else {
            return;
        };
        self.remember(job, worker, Standing::of(&state));
    }

    fn remember_transcript(&self, job: u64, worker: &TrackedWorker) {
        let Some(crew) = self.crew().filter(|_| worker.kept) else {
            return;
        };
        if let Some(transcript) = &worker.transcript {
            crew.keep_transcript(job, transcript.thread());
        }
    }

    fn following(&self) -> Option<Following> {
        self.inner.following.lock().clone()
    }

    fn wake(&self, job: u64) {
        if let Some(following) = self.following() {
            following(job);
        }
    }

    fn replied(&self, job: u64, reply: &Reply) {
        let mut workers = self.inner.workers.lock();
        if let Some(worker) = workers.get_mut(&job) {
            let model = worker.model().to_owned();
            let changed = worker
                .transcript
                .as_mut()
                .is_some_and(|transcript| transcript.apply(reply, &model));
            if changed {
                self.remember_transcript(job, worker);
            }
        }
        drop(workers);
        self.wake(job);
    }

    fn briefing(&self, hired: &ConnectedModel) -> Vec<String> {
        self.inner
            .briefing
            .iter()
            .map(|instruction| applied(instruction, hired.name()))
            .collect()
    }

    fn agent(
        &self,
        run_context: &RunContext,
        ticket: &Ticket,
        cancel: CancelToken,
        toolbox: &Toolbox,
        hired: &ConnectedModel,
        instructions: &[String],
    ) -> MessageResult<Agent> {
        let writing = ticket.writer();
        let subagents = self.clone();
        let job = ticket.id();

        Fitting {
            hired,
            toolbox,
            key: format!("{}-{}", run_context.key, job),
            budget: self.inner.budget,
            folding: Some(self.inner.folding.clone()),
            keep: Compaction::KEEP,
            instructions,
        }
        .agent(move |event: Event| {
            match &event {
                Event::Text(text) => writing.append_output(text),
                Event::ToolStarted { name, .. } => writing.append_output(&format!("\n[{name}]\n")),
                Event::Hosted(HostedPart::Call { tool, .. }) => {
                    writing.append_output(&format!("\n[{}]\n", tool.label()));
                }
                _ => {}
            }
            if let Some(reply) = reply_for(event) {
                subagents.replied(job, &reply);
            }
        })
        .wait_for_notices(true)
        .cancel_token(cancel)
        .build()
        .map_err(|error| error.to_string())
    }
}

fn planned(
    run_context: &RunContext,
    jobs: &Jobs,
    workers: &Workers,
    task_request: TaskRequest,
) -> Plan {
    let TaskRequest {
        label,
        task,
        job,
        model,
        effort,
    } = task_request;

    let Some(job) = job else {
        return match hired(&run_context.hired, model.as_deref(), effort) {
            Ok(hired) => Plan::Fresh { label, task, hired },
            Err(trouble) => Plan::Refused(trouble),
        };
    };
    let Some(worker) = workers.get(&job) else {
        return Plan::Refused(format!(
            "job {job} is not an agent this conversation started, so there is nothing to carry on"
        ));
    };

    if worker.running {
        if !jobs.state(job).is_some_and(|state| state.is_running()) {
            return Plan::Refused(format!(
                "job {job} is being stopped; give it the task again once it has ended"
            ));
        }
        if model.is_some() || effort.is_some() {
            return Plan::Refused(format!(
                "job {job} is still running, and a running agent stays on the model it was \
                 started on; send it the task alone, or start another agent"
            ));
        }
        return Plan::Send { job, task };
    }

    let current = match &worker.hired {
        Some(hired) => hired.clone(),
        None if model.is_some() => run_context.hired.clone(),
        None => {
            let conversation = &run_context.hired;
            let named = (worker.on.model() != conversation.name()).then_some(worker.on.model());
            match hired(conversation, named, worker.on.effort()) {
                Ok(hired) => hired,
                Err(trouble) => return Plan::Refused(trouble),
            }
        }
    };
    match hired(&current, model.as_deref(), effort) {
        Ok(hired) => Plan::Resume { job, task, hired },
        Err(trouble) => Plan::Refused(trouble),
    }
}

fn running_on(hired: &ConnectedModel) -> On {
    On::new(hired.name(), hired.effort())
}

fn hired(
    current: &ConnectedModel,
    model: Option<&str>,
    effort: Option<Effort>,
) -> MessageResult<ConnectedModel> {
    let Some(named) = model else {
        return Ok(match effort {
            Some(effort) => current.with_effort(effort),
            None => current.clone(),
        });
    };

    current
        .roster()
        .connect_named(named, effort)
        .map_err(|error| error.to_string())
}

fn named_once(plans: &[Plan]) -> MessageResult<()> {
    let mut named = BTreeSet::new();
    for job in plans.iter().filter_map(Plan::job) {
        if !named.insert(job) {
            return Err(format!(
                "job {job} is named twice in one call; give an agent one task at a time"
            ));
        }
    }
    Ok(())
}

fn sent(workers: &Workers, job: u64, task: &str) -> Started {
    let worker = workers.get(&job).expect("planned job should exist");
    if let Some(toolbox) = &worker.toolbox {
        toolbox.jobs().notices().post(Notice::new(message(task)));
    }
    worker
        .writer
        .append_output(&carried(DispatchKind::Sent, task));

    Started {
        label: worker.label.clone(),
        job,
        model: worker
            .hired
            .as_ref()
            .map_or_else(|| worker.on.model().to_owned(), ConnectedModel::label),
        kind: DispatchKind::Sent,
    }
}

fn message(task: &str) -> String {
    format!(
        "<message_from_the_agent_that_started_you>\n{task}\n</message_from_the_agent_that_started_you>"
    )
}

fn carried(kind: DispatchKind, task: &str) -> String {
    format!("\n[{}]\n{task}\n", kind.as_str())
}

fn ended(outcome: &rikka_agent::Result<Outcome>) -> Reply {
    match outcome {
        Ok(_) => Reply::Done,
        Err(AgentError::Cancelled) => Reply::Stopped,
        Err(refused) => Reply::Failed(Trouble::from(refused)),
    }
}

fn reported(ticket: Ticket, outcome: rikka_agent::Result<Outcome>) -> String {
    match outcome {
        Err(refused) => {
            ticket.finish("", Some(&refused.to_string()), Usage::default());
            String::new()
        }
        Ok(outcome) => {
            let usage = outcome.usage();
            let text = outcome.text().trim().to_owned();
            let trouble = trouble(&text, outcome.exhausted());
            let report = framed(&text);
            ticket.finish(&report, trouble.as_deref(), usage);
            report
        }
    }
}

fn trouble(text: &str, exhausted: Option<Limit>) -> Option<String> {
    match (text.is_empty(), exhausted) {
        (true, None) => Some("it finished without a report".to_owned()),
        (true, Some(limit)) => Some(format!("it {}, and it never reported", limit.reason())),
        (false, None) => None,
        (false, Some(limit)) => Some(format!(
            "it {}, so this is as far as it got",
            limit.reason()
        )),
    }
}

fn framed(text: &str) -> String {
    match text.is_empty() {
        true => String::new(),
        false => format!("\n<report>\n{text}\n</report>"),
    }
}
