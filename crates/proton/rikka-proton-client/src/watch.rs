use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::Stream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::event_loop::Change;
use crate::runner::Runner;

pub struct Watch<C> {
    changes: mpsc::Receiver<Change<C>>,
    task: Task<C>,
}

enum Task<C> {
    Pending(Option<Box<Runner<C>>>),
    Running(JoinHandle<()>),
}

impl<C: Send + 'static> Watch<C> {
    pub(crate) fn new(changes: mpsc::Receiver<Change<C>>, runner: Runner<C>) -> Watch<C> {
        Watch {
            changes,
            task: Task::Pending(Some(Box::new(runner))),
        }
    }

    pub async fn next(&mut self) -> Option<Change<C>> {
        self.spawn_runner();
        self.changes.recv().await
    }

    fn spawn_runner(&mut self) {
        let Task::Pending(runner) = &mut self.task else {
            return;
        };
        if let Some(runner) = runner.take() {
            self.task = Task::Running(tokio::spawn(runner.run()));
        }
    }
}

impl<C: Send + 'static> Stream for Watch<C> {
    type Item = Change<C>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Change<C>>> {
        self.spawn_runner();
        self.changes.poll_recv(context)
    }
}

impl<C> Drop for Watch<C> {
    fn drop(&mut self) {
        if let Task::Running(task) = &self.task {
            task.abort();
        }
    }
}

impl<C> fmt::Debug for Watch<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Watch").finish_non_exhaustive()
    }
}
