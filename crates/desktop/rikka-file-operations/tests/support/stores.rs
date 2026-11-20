use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use rikka_file_operations::{Entry, Local, Result, Sink, Store};

pub(crate) struct Elsewhere;

pub(crate) struct Gate {
    reading: Sender<()>,
    resume: Mutex<Option<Receiver<()>>>,
}

pub(crate) struct Gated {
    pub(crate) reading: Receiver<()>,
    pub(crate) resume: Sender<()>,
}

impl Gate {
    pub(crate) fn new() -> (Self, Gated) {
        let (reading, reading_seen) = channel();
        let (resume, resumed) = channel();
        let gate = Self {
            reading,
            resume: Mutex::new(Some(resumed)),
        };
        let gated = Gated {
            reading: reading_seen,
            resume,
        };
        (gate, gated)
    }
}

struct Paused {
    source: Box<dyn Read + Send>,
    reads: u32,
    reading: Sender<()>,
    resume: Option<Receiver<()>>,
}

impl Read for Paused {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reads += 1;
        if self.reads == 2
            && let Some(resume) = self.resume.take()
        {
            self.reading.send(()).expect("the test listens");
            resume.recv().expect("the test resumes");
        }
        self.source.read(buffer)
    }
}

macro_rules! through_local {
    ($store:ty, |$this:ident, $at:ident| $read:expr, |$from:ident, $to:ident| $renames:expr) => {
        impl Store for $store {
            type Location = PathBuf;
            type Item = Entry;

            fn stat(&self, at: &PathBuf) -> Result<Entry, PathBuf> {
                Local.stat(at)
            }

            fn walk(&self, at: &PathBuf) -> Result<Vec<Entry>, PathBuf> {
                Local.walk(at)
            }

            fn create_directory(&self, at: &PathBuf) -> Result<(), PathBuf> {
                Local.create_directory(at)
            }

            fn delete(&self, at: &PathBuf) -> Result<(), PathBuf> {
                Local.delete(at)
            }

            fn trash(&self, at: &PathBuf) -> Result<PathBuf, PathBuf> {
                Local.trash(at)
            }

            fn restore(&self, at: &PathBuf) -> Result<PathBuf, PathBuf> {
                Local.restore(at)
            }

            fn read(&self, $at: &PathBuf) -> Result<Box<dyn Read + Send>, PathBuf> {
                let $this = self;
                $read
            }

            fn write(&self, at: &PathBuf) -> Result<Box<dyn Sink>, PathBuf> {
                Local.write(at)
            }

            fn link(&self, target: &Path, at: &PathBuf) -> Result<(), PathBuf> {
                Local.link(target, at)
            }

            fn copy_metadata(&self, from: &Entry, to: &PathBuf) -> Result<(), PathBuf> {
                Local.copy_metadata(from, to)
            }

            fn can_rename_across(&self, $from: &PathBuf, $to: &PathBuf) -> bool {
                $renames
            }

            fn rename_across(&self, from: &PathBuf, to: &PathBuf) -> Result<(), PathBuf> {
                Local.rename_across(from, to)
            }
        }
    };
}

through_local!(Elsewhere, |_store, at| Local.read(at), |_from, _to| false);

through_local!(
    Gate,
    |gate, at| {
        let source = Local.read(at)?;
        let resume = gate.resume.lock().expect("the gate").take();
        Ok(Box::new(Paused {
            source,
            reads: 0,
            reading: gate.reading.clone(),
            resume,
        }))
    },
    |from, to| Local.can_rename_across(from, to)
);
