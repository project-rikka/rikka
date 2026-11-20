use std::io::ErrorKind;
use std::path::PathBuf;

use futures_util::{Stream, StreamExt};
use rikka_file_operations::{Entry, Operation, Outcome, Resolution, Snapshot};

use crate::error::Error;
use crate::root::Moved;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Moved(Vec<Moved>),
    Copied(Vec<PathBuf>),
    Removed(Vec<PathBuf>),
}

#[derive(Debug)]
pub struct Finished {
    pub change: Change,
    pub failures: Vec<Error>,
    pub cancelled: bool,
}

#[derive(Debug)]
pub enum Update {
    Working(Box<Snapshot<Entry>>),
    Finished(Finished),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Move,
    Copy,
    Trash,
    Delete,
}

#[derive(Debug)]
pub struct Job {
    operation: Operation<Entry>,
    kind: JobKind,
    asked: Vec<PathBuf>,
}

impl Job {
    pub(crate) fn new(operation: Operation<Entry>, kind: JobKind, asked: &[PathBuf]) -> Job {
        Job {
            operation,
            kind,
            asked: asked.to_vec(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> JobKind {
        self.kind
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot<Entry> {
        self.operation.snapshot()
    }

    pub fn updates(&self) -> impl Stream<Item = Update> + Send + 'static + use<> {
        let (kind, asked) = (self.kind, self.asked.clone());
        self.operation
            .updates()
            .map(move |snapshot| match snapshot.outcome {
                Some(outcome) => Update::Finished(settled(kind, &asked, outcome)),
                None => Update::Working(Box::new(snapshot)),
            })
    }

    pub fn resolve(&self, resolution: Resolution, remember: bool) {
        self.operation.resolve(resolution, remember);
    }

    pub fn cancel(&self) {
        self.operation.cancel();
    }

    pub fn wait(&self) -> Finished {
        settled(self.kind, &self.asked, self.operation.wait())
    }
}

fn settled(kind: JobKind, asked: &[PathBuf], outcome: Outcome<PathBuf>) -> Finished {
    let placed = outcome.placed;
    let change = match kind {
        JobKind::Move => Change::Moved(
            placed
                .into_iter()
                .map(|placement| Moved {
                    from: placement.from,
                    to: placement.to,
                })
                .collect(),
        ),
        JobKind::Copy => Change::Copied(placed.into_iter().map(|placement| placement.to).collect()),
        JobKind::Trash => {
            Change::Removed(placed.into_iter().map(|placement| placement.from).collect())
        }
        JobKind::Delete => Change::Removed(
            asked
                .iter()
                .filter(|path| {
                    path.symlink_metadata()
                        .is_err_and(|error| error.kind() == ErrorKind::NotFound)
                })
                .cloned()
                .collect(),
        ),
    };
    Finished {
        change,
        failures: outcome
            .failures
            .into_iter()
            .map(|failure| Error::Operation(failure.error))
            .collect(),
        cancelled: outcome.cancelled,
    }
}
