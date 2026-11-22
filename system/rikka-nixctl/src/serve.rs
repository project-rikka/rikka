use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_io::Timer;
use futures_channel::mpsc::{self, UnboundedReceiver};
use futures_channel::oneshot;
use futures_util::StreamExt as _;
use futures_util::future::{Either, select};
use nixctl_task::Site;
use rikka_nix_journal::{Journal, Transcript, Written};
use rikka_nix_protocol::{
    ChangeState, OBJECT, Request, RunId, RunRecord, RunState, SERVICE, Status, StatusRecord,
};
use zbus::connection::Builder;
use zbus::object_server::SignalEmitter;

use crate::error::{Error, LOGS_DIRECTORY, Result};
use crate::recorder::{Note, Recorder};
use crate::service::{Accepted, Nix, Shared};

const EXIT_WHEN_IDLE_FOR: Duration = Duration::from_secs(30);

const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

const BATCH_BYTES: usize = 256 * 1024;

const PANICKED: &str = "the rikka-nixctl task stopped unexpectedly";

pub(crate) async fn run() -> Result<()> {
    let folder = std::env::var_os(LOGS_DIRECTORY).ok_or(Error::LogsDirectory)?;
    let opened = Journal::open(PathBuf::from(folder))?;
    for rejected in &opened.rejected {
        eprintln!("skipping a run record: {rejected}");
    }
    let journal = Shared::new(opened.journal);

    let (sender, requests) = mpsc::unbounded();
    let running = Arc::new(AtomicBool::new(false));

    let connection = Builder::system()
        .map_err(Error::Bus)?
        .name(SERVICE)
        .and_then(|builder| {
            builder.serve_at(
                OBJECT,
                Nix::new(sender, Arc::clone(&running), journal.clone()),
            )
        })
        .map_err(Error::NameRequest)?
        .build()
        .await
        .map_err(Error::NameRequest)?;

    let emitter = SignalEmitter::new(&connection, OBJECT).map_err(Error::SignalEmitter)?;

    serve_requests(requests, &emitter, &running, &journal).await;
    Ok(())
}

async fn serve_requests(
    mut requests: UnboundedReceiver<Accepted>,
    emitter: &SignalEmitter<'_>,
    running: &AtomicBool,
    journal: &Shared,
) {
    loop {
        match select(requests.next(), Timer::after(EXIT_WHEN_IDLE_FOR)).await {
            Either::Left((Some(accepted), _)) => execute(accepted, emitter, journal).await,
            Either::Left((None, _)) => return,
            Either::Right(_) if running.load(Ordering::Acquire) => {}
            Either::Right(_) => return,
        }
    }
}

async fn execute(accepted: Accepted, emitter: &SignalEmitter<'_>, journal: &Shared) {
    let Accepted {
        request,
        site,
        run,
        transcript,
        guard,
    } = accepted;
    let state = record(run, request, site, transcript, emitter, journal).await;
    finish(run, state, emitter, journal).await;
    drop(guard);
}

async fn record(
    run: RunId,
    request: Request,
    site: Site,
    transcript: Transcript,
    emitter: &SignalEmitter<'_>,
    journal: &Shared,
) -> RunState {
    let (notes_sender, mut notes) = mpsc::unbounded();
    let (ended, ending) = oneshot::channel();
    let worker = std::thread::Builder::new()
        .name("rikka-nixctl-task".to_owned())
        .spawn(move || {
            let mut recorder = Recorder::new(transcript, notes_sender);
            let ending = nixctl_task::execute(request, &site, &mut recorder);
            if ended.send(recorder.conclude(ending)).is_err() {
                eprintln!("run state receiver closed");
            }
        });
    if let Err(error) = worker {
        return RunState::Failed {
            reason: format!("failed to spawn task thread: {error}"),
            changes: ChangeState::Untouched,
        };
    }

    let mut flushing = Flushing::new(run);
    let mut ticks = Timer::interval(FLUSH_INTERVAL);
    loop {
        match select(notes.next(), ticks.next()).await {
            Either::Left((Some(Note::Line(written)), _)) => {
                if flushing.push(written) >= BATCH_BYTES {
                    flushing.flush(emitter, journal).await;
                }
            }
            Either::Left((Some(Note::Status(status)), _)) => flushing.status = Some(status),
            Either::Left((None, _)) => break,
            Either::Right(_) => flushing.flush(emitter, journal).await,
        }
    }
    flushing.flush(emitter, journal).await;

    ending.await.unwrap_or_else(|_| RunState::Failed {
        reason: PANICKED.to_owned(),
        changes: ChangeState::Modified,
    })
}

async fn finish(run: RunId, state: RunState, emitter: &SignalEmitter<'_>, journal: &Shared) {
    let record = {
        let mut journal = journal.lock();
        if let Err(error) = journal.finish(run, state) {
            eprintln!("failed to record the end of run {run}: {error}");
        }
        journal.run(run).map(RunRecord::from)
    };
    let Some(record) = record else {
        return eprintln!("run {run} is not in the journal");
    };
    if let Err(error) = Nix::finished(emitter, record).await {
        eprintln!("failed to emit Finished: {error}");
    }
}

struct Flushing {
    run: RunId,
    from: u64,
    lines: Vec<String>,
    bytes: usize,
    status: Option<Status>,
}

impl Flushing {
    fn new(run: RunId) -> Self {
        Self {
            run,
            from: 0,
            lines: Vec::new(),
            bytes: 0,
            status: None,
        }
    }

    fn push(&mut self, written: Written) -> usize {
        if self.lines.is_empty() {
            self.from = written.offset;
        }
        self.bytes += written.text.len() + 1;
        self.lines.push(written.text);
        self.bytes
    }

    async fn flush(&mut self, emitter: &SignalEmitter<'_>, journal: &Shared) {
        if !self.lines.is_empty() {
            let lines = std::mem::take(&mut self.lines);
            self.bytes = 0;
            if let Err(error) = Nix::lines(emitter, self.run.get(), self.from, &lines).await {
                eprintln!("failed to emit Lines: {error}");
            }
        }
        if let Some(status) = self.status.take() {
            if let Err(error) = journal.lock().update(self.run, status) {
                eprintln!("failed to record the status of run {}: {error}", self.run);
            }
            if let Err(error) =
                Nix::status(emitter, self.run.get(), StatusRecord::from(status)).await
            {
                eprintln!("failed to emit Status: {error}");
            }
        }
    }
}
