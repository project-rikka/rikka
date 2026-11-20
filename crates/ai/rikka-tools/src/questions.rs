use rikka_sync::Mutex;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::oneshot;

use crate::question::Question;
use crate::question_set::QuestionSet;
use crate::selection::Selection;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestionEvent {
    Posted(QuestionSet),
    Withdrawn(u64),
}

type Watcher = Arc<dyn Fn(QuestionEvent) + Send + Sync>;

#[derive(Clone, Debug)]
pub struct Questions {
    inner: Arc<Inner>,
}

struct Inner {
    next_id: AtomicU64,
    pending: Mutex<Pending>,
    watcher: Mutex<Option<Watcher>>,
}

struct Pending {
    open: bool,
    waiting: BTreeMap<u64, oneshot::Sender<Vec<Selection>>>,
}

impl Default for Questions {
    fn default() -> Questions {
        Questions::new()
    }
}

impl Questions {
    #[must_use]
    pub fn new() -> Questions {
        Questions {
            inner: Arc::new(Inner {
                next_id: AtomicU64::new(0),
                pending: Mutex::new(Pending {
                    open: true,
                    waiting: BTreeMap::new(),
                }),
                watcher: Mutex::new(None),
            }),
        }
    }

    pub fn watch(&self, watcher: impl Fn(QuestionEvent) + Send + Sync + 'static) {
        *self.inner.watcher.lock() = Some(Arc::new(watcher));
    }

    pub fn reopen(&self) {
        self.inner.pending.lock().open = true;
    }

    pub fn answer(&self, id: u64, selections: Vec<Selection>) {
        let waiting = self.inner.pending.lock().waiting.remove(&id);
        if let Some(waiting) = waiting {
            let _answered = waiting.send(selections);
        }
    }

    pub fn withdraw(&self) {
        let withdrawn = {
            let mut pending = self.inner.pending.lock();
            pending.open = false;
            std::mem::take(&mut pending.waiting)
        };
        for id in withdrawn.into_keys() {
            self.notify(QuestionEvent::Withdrawn(id));
        }
    }

    pub async fn ask(&self, questions: Vec<Question>) -> Option<Vec<Selection>> {
        if questions
            .iter()
            .any(|question| question.validation_error().is_some())
        {
            return None;
        }
        self.put(questions).answer().await
    }

    pub(crate) fn put(&self, questions: Vec<Question>) -> Waiting {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, answer) = oneshot::channel();

        let put = {
            let mut pending = self.inner.pending.lock();
            if pending.open {
                pending.waiting.insert(id, sender);
            }
            pending.open
        };
        if put {
            self.notify(QuestionEvent::Posted(QuestionSet::new(id, questions)));
        }

        Waiting {
            questions: self.clone(),
            id,
            answer,
        }
    }

    fn notify(&self, event: QuestionEvent) {
        let watcher = self.inner.watcher.lock().clone();
        if let Some(watcher) = watcher {
            watcher(event);
        }
    }
}

#[derive(Debug)]
pub(crate) struct Waiting {
    questions: Questions,
    id: u64,
    answer: oneshot::Receiver<Vec<Selection>>,
}

impl Waiting {
    pub(crate) async fn answer(&mut self) -> Option<Vec<Selection>> {
        (&mut self.answer).await.ok()
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let waiting = self
            .questions
            .inner
            .pending
            .lock()
            .waiting
            .remove(&self.id)
            .is_some();
        if waiting {
            self.questions.notify(QuestionEvent::Withdrawn(self.id));
        }
    }
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Questions")
            .field("waiting", &self.pending.lock().waiting.len())
            .finish_non_exhaustive()
    }
}
