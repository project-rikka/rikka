mod files;
mod panics;

use rikka_sync::Mutex;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

use crate::crash::Tail;

pub(crate) use files::{open_shell_log, state_directory};

const DEFAULT_FILTER: &str = "info,rikka_compositor=debug";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Journal {
    Session,
    Nested,
    Headless,
}

impl Journal {
    fn backend(self) -> Option<&'static str> {
        match self {
            Journal::Session => None,
            Journal::Nested => Some("nested"),
            Journal::Headless => Some("headless"),
        }
    }
}

#[derive(Debug)]
pub struct Logging {
    journal: Journal,
    journaled: Arc<Mutex<Journaled>>,
}

#[derive(Debug)]
struct Journaled {
    sink: Sink,
    path: Option<PathBuf>,
    tail: Tail,
}

#[derive(Debug)]
enum Sink {
    Pending(Vec<u8>),
    Open(std::fs::File),
    Closed,
}

pub fn install_logging(journal: Journal) -> Logging {
    let (sink, path, failure) = match journal.backend() {
        None => match files::open_session_log() {
            Ok((path, file)) => (Sink::Open(file), Some(path), None),
            Err(error) => (Sink::Closed, None, Some(error)),
        },
        Some(_) => (Sink::Pending(Vec::new()), None, None),
    };
    let journaled = Arc::new(Mutex::new(Journaled {
        sink,
        path,
        tail: Tail::default(),
    }));
    let terminal = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(filter());
    let file = fmt::layer()
        .with_ansi(false)
        .with_writer(Shared(journaled.clone()))
        .with_filter(filter());
    tracing_subscriber::registry()
        .with(terminal)
        .with(file)
        .init();
    panics::report(journaled.clone());
    if let Some(error) = failure {
        tracing::warn!("failed to open compositor log: {error}");
    }
    Logging { journal, journaled }
}

impl Logging {
    pub fn name_after(&self, socket: &str) -> Option<PathBuf> {
        let Some(backend) = self.journal.backend() else {
            return self.journaled.lock().path.clone();
        };
        let opened = files::open_instance_log(backend, socket);
        let mut journaled = self.journaled.lock();
        let Sink::Pending(buffered) = std::mem::replace(&mut journaled.sink, Sink::Closed) else {
            return None;
        };
        let written = opened.and_then(|(path, mut file)| {
            file.write_all(&buffered)?;
            Ok((path, file))
        });
        match written {
            Ok((path, file)) => {
                journaled.sink = Sink::Open(file);
                journaled.path = Some(path.clone());
                Some(path)
            }
            Err(error) => {
                drop(journaled);
                tracing::warn!("failed to open compositor log: {error}");
                None
            }
        }
    }
}

struct Shared(Arc<Mutex<Journaled>>);

impl<'a> MakeWriter<'a> for Shared {
    type Writer = Entry<'a>;

    fn make_writer(&'a self) -> Entry<'a> {
        Entry(&self.0)
    }
}

struct Entry<'a>(&'a Mutex<Journaled>);

impl Write for Entry<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut journaled = self.0.lock();
        let written = match &mut journaled.sink {
            Sink::Pending(buffered) => {
                buffered.extend_from_slice(bytes);
                bytes.len()
            }
            Sink::Open(file) => file.write(bytes)?,
            Sink::Closed => bytes.len(),
        };
        journaled.tail.push(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.0.lock().sink {
            Sink::Open(file) => file.flush(),
            Sink::Pending(_) | Sink::Closed => Ok(()),
        }
    }
}

fn filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER))
}
