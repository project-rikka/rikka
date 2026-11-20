use std::sync::Arc;

use crate::item::Item;
use crate::operation::{Operation, Outcome, Phase, Shared, TaskKind};

const THREAD: &str = "rikka-file-operation";

#[derive(Debug)]
pub struct Reporter<I: Item> {
    shared: Arc<Shared<I>>,
}

impl<I: Item> Clone for Reporter<I> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<I: Item> Reporter<I> {
    pub fn totals(&self, items: u64, bytes: u64) {
        self.shared.update(|inner| {
            inner.progress.phase = Phase::Working;
            inner.progress.items_total = items;
            inner.progress.bytes_total = bytes;
        });
    }

    pub fn reached(&self, items_done: u64, bytes_done: u64) {
        self.shared.update(|inner| {
            inner.progress.items_done = items_done;
            inner.progress.bytes_done = bytes_done;
        });
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.shared.is_cancelled()
    }

    pub(crate) fn shared(&self) -> &Arc<Shared<I>> {
        &self.shared
    }
}

#[must_use]
pub fn spawn<I: Item>(
    kind: TaskKind,
    work: impl FnOnce(&Reporter<I>) -> Outcome<I::Location> + Send + 'static,
) -> Operation<I> {
    let shared = Arc::new(Shared::default());
    let worker = Arc::clone(&shared);

    let spawned = std::thread::Builder::new()
        .name(THREAD.to_owned())
        .spawn(move || {
            let outcome = work(&Reporter {
                shared: Arc::clone(&worker),
            });
            worker.update(|inner| {
                inner.progress.phase = Phase::Done;
                inner.question = None;
                inner.outcome = Some(outcome);
            });
        });

    if let Err(error) = spawned {
        tracing::error!(%error, "failed to spawn file operation thread");
        shared.update(|inner| {
            inner.progress.phase = Phase::Done;
            inner.outcome = Some(Outcome {
                cancelled: true,
                ..Outcome::default()
            });
        });
    }
    Operation::new(shared, kind)
}
