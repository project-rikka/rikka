use rikka_sync::Mutex;
use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

use rusqlite::{Connection, TransactionBehavior};

use crate::error::{Error, Result};
use crate::ids::{DownloadId, ProfileId};
use crate::session::{Session, SessionCache};

const WRITE_BEHIND: Duration = Duration::from_secs(1);
const MAX_BATCH: usize = 2048;

pub(crate) type Job = Box<dyn FnOnce(&Connection, &mut WriterState) -> Result<()> + Send>;
pub(crate) type Call =
    Box<dyn FnOnce(Result<(&Connection, &mut WriterState), Error>) -> CallOutcome + Send>;
pub(crate) type Callback = Box<dyn FnOnce(Result<()>) + Send>;

pub(crate) struct CallOutcome {
    pub(crate) succeeded: bool,
    pub(crate) deliver: Callback,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Coalesce {
    Title(ProfileId, String),
    Favicon(String),
    Zoom(ProfileId, String),
    Download(DownloadId),
    ProfileSettings(ProfileId),
}

pub(crate) enum Message {
    Write(Job),
    Behind(Option<Coalesce>, Job),
    Call(Call),
    Session,
    Flush(SyncSender<Result<()>>),
    Shutdown,
}

pub(crate) type SessionSlot = Arc<Mutex<Option<Session>>>;

#[derive(Default)]
pub(crate) struct WriterState {
    pub(crate) session: SessionCache,
    failed: usize,
    first_failure: Option<Error>,
}

impl WriterState {
    fn fail(&mut self, error: Error) {
        tracing::error!(%error, "browser store write failed");
        self.failed += 1;
        self.first_failure.get_or_insert(error);
    }

    fn take_failures(&mut self) -> Result<()> {
        let count = mem::take(&mut self.failed);
        match self.first_failure.take() {
            Some(first) => Err(Error::WritesFailed {
                count,
                first: Box::new(first),
            }),
            None => Ok(()),
        }
    }
}

enum Entry {
    Job(Job),
    Call(Call),
    Session,
}

#[derive(Default)]
struct Batch {
    entries: Vec<Option<Entry>>,
    coalesced: HashMap<Coalesce, usize>,
    flushes: Vec<SyncSender<Result<()>>>,
    urgent: bool,
    shutdown: bool,
}

impl Batch {
    fn push(&mut self, message: Message) {
        match message {
            Message::Write(job) => {
                self.urgent = true;
                self.entries.push(Some(Entry::Job(job)));
            }
            Message::Behind(key, job) => {
                if let Some(key) = key
                    && let Some(previous) = self.coalesced.insert(key, self.entries.len())
                {
                    self.entries[previous] = None;
                }
                self.entries.push(Some(Entry::Job(job)));
            }
            Message::Call(call) => {
                self.urgent = true;
                self.entries.push(Some(Entry::Call(call)));
            }
            Message::Session => self.entries.push(Some(Entry::Session)),
            Message::Flush(reply) => {
                self.urgent = true;
                self.flushes.push(reply);
            }
            Message::Shutdown => {
                self.urgent = true;
                self.shutdown = true;
            }
        }
    }

    fn has_room(&self) -> bool {
        !self.shutdown && self.entries.len() < MAX_BATCH
    }

    fn ready(&self) -> bool {
        self.urgent || !self.has_room()
    }
}

pub(crate) struct Writer {
    connection: Connection,
    messages: Receiver<Message>,
    session: SessionSlot,
    state: WriterState,
}

impl Writer {
    pub(crate) fn new(
        connection: Connection,
        messages: Receiver<Message>,
        session: SessionSlot,
    ) -> Self {
        Self {
            connection,
            messages,
            session,
            state: WriterState::default(),
        }
    }

    pub(crate) fn run(mut self) {
        while let Ok(first) = self.messages.recv() {
            let batch = self.gather(first);
            let shutdown = batch.shutdown;
            self.commit(batch);
            if shutdown {
                break;
            }
        }
        if let Err(error) = self.connection.execute_batch("PRAGMA optimize") {
            tracing::warn!(%error, "browser store optimize failed");
        }
    }

    fn gather(&self, first: Message) -> Batch {
        let deadline = Instant::now() + WRITE_BEHIND;
        let mut batch = Batch::default();
        batch.push(first);
        loop {
            while batch.has_room() {
                match self.messages.try_recv() {
                    Ok(message) => batch.push(message),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        batch.shutdown = true;
                        return batch;
                    }
                }
            }
            if batch.ready() {
                return batch;
            }
            let wait = deadline.saturating_duration_since(Instant::now());
            match self.messages.recv_timeout(wait) {
                Ok(message) => batch.push(message),
                Err(RecvTimeoutError::Timeout) => return batch,
                Err(RecvTimeoutError::Disconnected) => {
                    batch.shutdown = true;
                    return batch;
                }
            }
        }
    }

    fn commit(&mut self, batch: Batch) {
        let mut deliveries = Vec::new();
        let committed = self.execute(batch.entries, &mut deliveries);
        if let Err(error) = &committed {
            self.state.session.invalidate();
            self.state.fail(error.clone());
        }
        for deliver in deliveries {
            deliver(committed.clone());
        }
        if batch.flushes.is_empty() {
            return;
        }
        let mut failures = self.state.take_failures();
        for reply in batch.flushes {
            let _ = reply.send(mem::replace(&mut failures, Ok(())));
        }
    }

    fn execute(
        &mut self,
        entries: Vec<Option<Entry>>,
        deliveries: &mut Vec<Callback>,
    ) -> Result<()> {
        let state = &mut self.state;
        let transaction = match self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
        {
            Ok(transaction) => transaction,
            Err(error) => {
                let error = Error::from(error);
                for entry in entries.into_iter().flatten() {
                    if let Entry::Call(call) = entry {
                        deliveries.push(call(Err(error.clone())).deliver);
                    }
                }
                return Err(error);
            }
        };
        for entry in entries.into_iter().flatten() {
            match entry {
                Entry::Job(job) => {
                    if let Err(error) = savepoint(&transaction, |c| job(c, state)) {
                        state.fail(error);
                    }
                }
                Entry::Call(call) => {
                    let settle = match begin(&transaction) {
                        Ok(()) => {
                            let settle = call(Ok((&transaction, state)));
                            if let Err(error) = end(&transaction, settle.succeeded) {
                                state.fail(error);
                            }
                            settle
                        }
                        Err(error) => call(Err(error)),
                    };
                    deliveries.push(settle.deliver);
                }
                Entry::Session => {
                    let session = self.session.lock().take();
                    if let Some(session) = session {
                        let cache = &mut state.session;
                        let written =
                            savepoint(&transaction, |c| crate::session::write(c, cache, &session));
                        if let Err(error) = written {
                            state.session.invalidate();
                            state.fail(error);
                        }
                    }
                }
            }
        }
        transaction.commit().map_err(Error::from)
    }
}

fn begin(connection: &Connection) -> Result<()> {
    connection.execute_batch("SAVEPOINT job")?;
    Ok(())
}

fn end(connection: &Connection, keep: bool) -> Result<()> {
    if keep {
        connection.execute_batch("RELEASE job")?;
    } else {
        connection.execute_batch("ROLLBACK TO job; RELEASE job")?;
    }
    Ok(())
}

fn savepoint(connection: &Connection, job: impl FnOnce(&Connection) -> Result<()>) -> Result<()> {
    begin(connection)?;
    let outcome = job(connection);
    end(connection, outcome.is_ok())?;
    outcome
}
