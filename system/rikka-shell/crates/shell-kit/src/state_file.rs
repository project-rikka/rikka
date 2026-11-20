use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};

use rikka_dirs::Paths;
use rikka_sync::{Condvar, Mutex, MutexGuard};
use serde::de::DeserializeOwned;

const THREAD: &str = "rikka-shell-state";

const UNREADABLE: &str = "unreadable";

#[derive(Clone, Debug)]
pub struct StateFile {
    path: Arc<Path>,
}

impl StateFile {
    #[must_use]
    pub fn state(name: &str) -> Option<StateFile> {
        StateFile::under(name, Paths::state_dir)
    }

    #[must_use]
    pub fn config(name: &str) -> Option<StateFile> {
        StateFile::under(name, Paths::config_dir)
    }

    fn under(name: &str, dir: fn(&Paths) -> &Path) -> Option<StateFile> {
        let paths = Paths::resolve()
            .inspect_err(|error| tracing::warn!(%error, name, "failed to resolve directories"))
            .ok()?;
        rikka_dirs::ensure(dir(&paths))
            .inspect_err(|error| tracing::warn!(%error, name, "failed to create directory"))
            .ok()?;
        Some(StateFile {
            path: Arc::from(dir(&paths).join(name)),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self) -> io::Result<Option<String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    #[must_use]
    pub fn load_json<T: DeserializeOwned + Default>(&self) -> T {
        let text = match self.read() {
            Ok(Some(text)) => text,
            Ok(None) => return T::default(),
            Err(error) => {
                tracing::warn!("failed to read `{}`: {error}", self.path.display());
                return T::default();
            }
        };
        serde_json::from_str(&text).unwrap_or_else(|malformed| {
            match self.set_aside(UNREADABLE, &text) {
                Ok(aside) => tracing::warn!(
                    "`{}` is malformed ({malformed}), backed up to `{}`",
                    self.path.display(),
                    aside.display()
                ),
                Err(error) => tracing::warn!(
                    "`{}` is malformed ({malformed}), failed to back it up: {error}",
                    self.path.display()
                ),
            }
            T::default()
        })
    }

    pub fn set_aside(&self, why: &str, text: &str) -> io::Result<PathBuf> {
        let mut aside = self.path.as_os_str().to_owned();
        aside.push(".");
        aside.push(why);
        let aside = PathBuf::from(aside);
        rikka_fs::replace(&aside, text)?;
        Ok(aside)
    }

    pub fn save(&self, document: String) {
        writer().queue(Arc::clone(&self.path), document);
    }

    pub fn flush(&self) {
        writer().flush(&self.path);
    }
}

struct Writer {
    state: Mutex<Queue>,
    queued: Condvar,
    written: Condvar,
}

struct Queue {
    pending: VecDeque<(Arc<Path>, String)>,
    writing: Option<Arc<Path>>,
    running: bool,
}

static WRITER: Writer = Writer {
    state: Mutex::new(Queue {
        pending: VecDeque::new(),
        writing: None,
        running: false,
    }),
    queued: Condvar::new(),
    written: Condvar::new(),
};

fn writer() -> &'static Writer {
    static START: Once = Once::new();
    START.call_once(|| {
        let started = std::thread::Builder::new()
            .name(THREAD.to_owned())
            .spawn(|| WRITER.run());
        match started {
            Ok(_) => WRITER.state().running = true,
            Err(error) => {
                tracing::error!("failed to start {THREAD}, writing synchronously: {error}")
            }
        }
    });
    &WRITER
}

impl Writer {
    fn queue(&self, path: Arc<Path>, document: String) {
        let mut state = self.state();
        if !state.running {
            drop(state);
            return write(&path, &document);
        }
        match state.pending.iter_mut().find(|(queued, _)| *queued == path) {
            Some((_, queued)) => *queued = document,
            None => state.pending.push_back((path, document)),
        }
        self.queued.notify_one();
    }

    fn flush(&self, path: &Path) {
        let mut state = self.state();
        while state.writing.as_deref() == Some(path)
            || state.pending.iter().any(|(queued, _)| &**queued == path)
        {
            state = self.written.wait(state);
        }
    }

    fn run(&self) {
        loop {
            let (path, document) = {
                let mut state = self.state();
                loop {
                    if let Some(next) = state.pending.pop_front() {
                        state.writing = Some(Arc::clone(&next.0));
                        break next;
                    }
                    state = self.queued.wait(state);
                }
            };
            write(&path, &document);
            self.state().writing = None;
            self.written.notify_all();
        }
    }

    fn state(&self) -> MutexGuard<'_, Queue> {
        self.state.lock()
    }
}

fn write(path: &Path, document: &str) {
    if let Err(error) = rikka_fs::replace(path, document) {
        tracing::error!("failed to write `{}`: {error}", path.display());
    }
}
