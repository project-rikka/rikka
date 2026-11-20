use rikka_sync::Mutex;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

use rusqlite::Connection;

use crate::connection::Location;
use crate::error::{Error, Result};
use crate::ids::{SplitGroupId, TabId};
use crate::schema;
use crate::sequence::Sequences;
use crate::session::Session;
use crate::writer::{CallOutcome, Coalesce, Message, SessionSlot, Writer, WriterState};

const IDLE_READERS: usize = 4;

#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

struct Inner {
    location: Location,
    messages: Sender<Message>,
    readers: Mutex<Vec<Connection>>,
    sequences: Sequences,
    session: SessionSlot,
    writer: Option<JoinHandle<()>>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::start(Location::file(path.as_ref())?)
    }

    pub fn in_memory() -> Result<Self> {
        Self::start(Location::memory())
    }

    fn start(location: Location) -> Result<Self> {
        let mut connection = location.open_writer()?;
        schema::prepare(&mut connection)?;
        let sequences = Sequences::load(&connection)?;
        let session = SessionSlot::default();
        let (messages, inbox) = mpsc::channel();
        let writer = Writer::new(connection, inbox, Arc::clone(&session));
        let writer = thread::Builder::new()
            .name("rikka-browser-store".into())
            .spawn(move || writer.run())
            .map_err(|error| Error::Open(error.to_string()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                location,
                messages,
                readers: Mutex::default(),
                sequences,
                session,
                writer: Some(writer),
            }),
        })
    }

    pub fn flush(&self) -> Result<()> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.send(Message::Flush(reply))?;
        answer.recv().map_err(|_| Error::WriterStopped)?
    }

    pub(crate) fn queue_session(&self, session: Session) {
        let previous = self.inner.session.lock().replace(session);
        if previous.is_none() {
            self.post(Message::Session);
        }
    }

    #[must_use]
    pub fn new_tab_id(&self) -> TabId {
        TabId::from_raw(self.inner.sequences.tab.next())
    }

    #[must_use]
    pub fn new_split_group_id(&self) -> SplitGroupId {
        SplitGroupId::from_raw(self.inner.sequences.split_group.next())
    }

    pub(crate) fn sequences(&self) -> &Sequences {
        &self.inner.sequences
    }

    pub(crate) fn read<T>(&self, query: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let idle = self.inner.readers.lock().pop();
        let connection = match idle {
            Some(connection) => connection,
            None => self.inner.location.open_reader()?,
        };
        let outcome = connection
            .unchecked_transaction()
            .map_err(Error::from)
            .and_then(|snapshot| query(&snapshot));
        let mut readers = self.inner.readers.lock();
        if readers.len() < IDLE_READERS {
            readers.push(connection);
        }
        outcome
    }

    pub(crate) fn write(
        &self,
        job: impl FnOnce(&Connection, &mut WriterState) -> Result<()> + Send + 'static,
    ) {
        self.post(Message::Write(Box::new(job)));
    }

    pub(crate) fn write_behind(
        &self,
        key: Option<Coalesce>,
        job: impl FnOnce(&Connection, &mut WriterState) -> Result<()> + Send + 'static,
    ) {
        self.post(Message::Behind(key, Box::new(job)));
    }

    pub(crate) fn call<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Connection, &mut WriterState) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.send(Message::Call(Box::new(move |context| {
            let outcome = context.and_then(|(connection, state)| job(connection, state));
            CallOutcome {
                succeeded: outcome.is_ok(),
                deliver: Box::new(move |committed| {
                    let _ = reply.send(committed.and(outcome));
                }),
            }
        })))?;
        answer.recv().map_err(|_| Error::WriterStopped)?
    }

    fn send(&self, message: Message) -> Result<()> {
        self.inner
            .messages
            .send(message)
            .map_err(|_| Error::WriterStopped)
    }

    fn post(&self, message: Message) {
        if self.send(message).is_err() {
            tracing::error!("browser store writer stopped, dropping write");
        }
    }
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store")
            .field("location", &self.inner.location)
            .finish_non_exhaustive()
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.messages.send(Message::Shutdown);
        if let Some(writer) = self.writer.take()
            && writer.join().is_err()
        {
            tracing::error!("browser store writer panicked");
        }
    }
}
