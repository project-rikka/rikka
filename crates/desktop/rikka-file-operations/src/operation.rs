use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::task::{Poll, Waker};

use futures_util::Stream;

use crate::address::Address;
use crate::error::Error;
use crate::item::Item;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Task<L: Address> {
    Copy { sources: Vec<L>, into: L },
    Move { sources: Vec<L>, into: L },
    Relocate { placements: Vec<Placement<L>> },
    Trash { items: Vec<L> },
    Delete { items: Vec<L> },
    Restore { items: Vec<L> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Copy,
    Move,
    Trash,
    Delete,
    Restore,
    Compress,
    Extract,
    Convert,
}

impl<L: Address> Task<L> {
    #[must_use]
    pub fn kind(&self) -> TaskKind {
        match self {
            Self::Copy { .. } => TaskKind::Copy,
            Self::Move { .. } | Self::Relocate { .. } => TaskKind::Move,
            Self::Trash { .. } => TaskKind::Trash,
            Self::Delete { .. } => TaskKind::Delete,
            Self::Restore { .. } => TaskKind::Restore,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement<L: Address> {
    pub from: L,
    pub to: L,
}

impl<L: Address> Placement<L> {
    #[must_use]
    pub fn new(from: L, to: L) -> Self {
        Self { from, to }
    }

    #[must_use]
    pub fn reversed(&self) -> Self {
        Self {
            from: self.to.clone(),
            to: self.from.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Preparing,
    Working,
    Asking,
    Done,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub phase: Phase,
    pub items_total: u64,
    pub items_done: u64,
    pub bytes_total: u64,
    pub bytes_done: u64,
}

impl Progress {
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        if self.phase == Phase::Preparing {
            return None;
        }
        if self.bytes_total > 0 {
            return Some(self.bytes_done as f32 / self.bytes_total as f32);
        }
        (self.items_total > 0).then(|| self.items_done as f32 / self.items_total as f32)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Conflict<I: Item> {
    pub source: I,
    pub existing: I,
    pub destination: I::Location,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Replace,
    Keep,
    Skip,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct Snapshot<I: Item> {
    pub progress: Progress,
    pub question: Option<Conflict<I>>,
    pub outcome: Option<Outcome<I::Location>>,
}

#[derive(Clone, Debug)]
pub struct Outcome<L: Address> {
    pub cancelled: bool,
    pub failures: Vec<Failure<L>>,
    pub placed: Vec<Placement<L>>,
}

impl<L: Address> Default for Outcome<L> {
    fn default() -> Self {
        Self {
            cancelled: false,
            failures: Vec::new(),
            placed: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Failure<L: Address> {
    pub at: L,
    pub error: Error<L>,
}

impl<L: Address> Failure<L> {
    #[must_use]
    pub fn new(at: L, error: Error<L>) -> Self {
        Self { at, error }
    }
}

pub(crate) struct Shared<I: Item> {
    inner: Mutex<Inner<I>>,
    answered: Condvar,
    changed: Condvar,
    cancelled: AtomicBool,
    revision: AtomicU64,
    wakers: Mutex<Vec<Waker>>,
}

impl<I: Item> Default for Shared<I> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            answered: Condvar::default(),
            changed: Condvar::default(),
            cancelled: AtomicBool::default(),
            revision: AtomicU64::default(),
            wakers: Mutex::default(),
        }
    }
}

impl<I: Item> Shared<I> {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner<I>> {
        self.inner.lock()
    }

    pub(crate) fn update(&self, change: impl FnOnce(&mut Inner<I>)) {
        change(&mut self.lock());
        self.revision.fetch_add(1, Ordering::Release);
        self.changed.notify_all();
        let wakers = std::mem::take(&mut *self.wakers.lock());
        for waker in wakers {
            waker.wake();
        }
    }

    pub(crate) fn wait_for_answer(&self) -> Resolution {
        let mut inner = self.lock();
        loop {
            if let Some(answer) = inner.answer.take() {
                return answer;
            }
            inner = self.answered.wait(inner);
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn register(&self, waker: &Waker) {
        let mut wakers = self.wakers.lock();
        if !wakers.iter().any(|known| known.will_wake(waker)) {
            wakers.push(waker.clone());
        }
    }
}

impl<I: Item> std::fmt::Debug for Shared<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("inner", &*self.lock())
            .field("cancelled", &self.is_cancelled())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) struct Inner<I: Item> {
    pub(crate) progress: Progress,
    pub(crate) question: Option<Conflict<I>>,
    pub(crate) answer: Option<Resolution>,
    pub(crate) standing: Option<Resolution>,
    pub(crate) outcome: Option<Outcome<I::Location>>,
}

impl<I: Item> Default for Inner<I> {
    fn default() -> Self {
        Self {
            progress: Progress::default(),
            question: None,
            answer: None,
            standing: None,
            outcome: None,
        }
    }
}

impl<I: Item> Inner<I> {
    fn snapshot(&self) -> Snapshot<I> {
        Snapshot {
            progress: self.progress.clone(),
            question: self.question.clone(),
            outcome: self.outcome.clone(),
        }
    }
}

#[derive(Debug)]
pub struct Operation<I: Item> {
    shared: Arc<Shared<I>>,
    kind: TaskKind,
}

impl<I: Item> Operation<I> {
    pub(crate) fn new(shared: Arc<Shared<I>>, kind: TaskKind) -> Self {
        Self { shared, kind }
    }

    #[must_use]
    pub fn kind(&self) -> TaskKind {
        self.kind
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot<I> {
        self.shared.lock().snapshot()
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.shared.lock().outcome.is_some()
    }

    pub fn updates(&self) -> impl Stream<Item = Snapshot<I>> + Send + 'static + use<I> {
        let shared = Arc::clone(&self.shared);
        let mut delivered = None;
        let mut finished = false;
        futures_util::stream::poll_fn(move |context| {
            if finished {
                return Poll::Ready(None);
            }
            shared.register(context.waker());
            let revision = shared.revision.load(Ordering::Acquire);
            if delivered == Some(revision) {
                return Poll::Pending;
            }
            delivered = Some(revision);
            let snapshot = shared.lock().snapshot();
            finished = snapshot.outcome.is_some();
            Poll::Ready(Some(snapshot))
        })
    }

    #[must_use]
    pub fn wait_until(&self, mut reached: impl FnMut(&Snapshot<I>) -> bool) -> Snapshot<I> {
        let mut inner = self.shared.lock();
        loop {
            let snapshot = inner.snapshot();
            if snapshot.outcome.is_some() || reached(&snapshot) {
                return snapshot;
            }
            inner = self.shared.changed.wait(inner);
        }
    }

    pub fn wait(&self) -> Outcome<I::Location> {
        let mut inner = self.shared.lock();
        loop {
            if let Some(outcome) = &inner.outcome {
                return outcome.clone();
            }
            inner = self.shared.changed.wait(inner);
        }
    }

    pub fn resolve(&self, resolution: Resolution, remember: bool) {
        {
            let mut inner = self.shared.lock();
            inner.answer = Some(resolution);
            if remember {
                inner.standing = Some(resolution);
            }
        }
        self.shared.answered.notify_all();
    }

    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Relaxed);
        self.resolve(Resolution::Cancel, true);
    }
}

impl<I: Item> Drop for Operation<I> {
    fn drop(&mut self) {
        if !self.is_finished() {
            self.cancel();
        }
    }
}
