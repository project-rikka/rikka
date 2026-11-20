mod connection;
mod schema;

use rikka_sync::Mutex;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use rusqlite::{Connection, Transaction};

use crate::error::{Error, Result};

const IDLE_READERS: usize = 4;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

pub(crate) struct Store {
    path: PathBuf,
    jobs: Mutex<Option<Sender<Job>>>,
    readers: Mutex<Vec<Connection>>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

impl Store {
    pub(crate) fn open(path: &Path) -> Result<Store> {
        let mut connection = connection::writer(path)?;
        schema::prepare(&mut connection)?;
        let (jobs, inbox) = mpsc::channel::<Job>();
        let writer = std::thread::Builder::new()
            .name("rikka-music-db".into())
            .spawn(move || {
                for job in inbox {
                    job(&mut connection);
                }
            })
            .map_err(|error| Error::Open(error.to_string()))?;
        Ok(Store {
            path: path.to_owned(),
            jobs: Mutex::new(Some(jobs)),
            readers: Mutex::default(),
            writer: Mutex::new(Some(writer)),
        })
    }

    pub(crate) fn write<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (reply, answer) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |connection| {
            let outcome = connection
                .transaction()
                .map_err(Error::from)
                .and_then(|transaction| {
                    let value = job(&transaction)?;
                    transaction.commit()?;
                    Ok(value)
                });
            let _ = reply.send(outcome);
        });
        self.jobs
            .lock()
            .as_ref()
            .ok_or(Error::Stopped)?
            .send(job)
            .map_err(|_| Error::Stopped)?;
        answer.recv().map_err(|_| Error::Stopped)?
    }

    pub(crate) fn read<T>(&self, query: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let idle = self.readers.lock().pop();
        let mut connection = match idle {
            Some(connection) => connection,
            None => connection::reader(&self.path)?,
        };
        let outcome = connection
            .transaction()
            .map_err(Error::from)
            .and_then(|snapshot| query(&snapshot));
        let mut readers = self.readers.lock();
        if readers.len() < IDLE_READERS {
            readers.push(connection);
        }
        outcome
    }

    pub(crate) fn stop(&self) {
        self.jobs.lock().take();
        let writer = self.writer.lock().take();
        if let Some(writer) = writer
            && writer.join().is_err()
        {
            tracing::error!("music library writer panicked");
        }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.stop();
    }
}

impl fmt::Debug for Store {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Store")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}
