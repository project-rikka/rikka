use rikka_sync::{Mutex, MutexGuard};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_picture::{Sleepers, Waking};

static MARKED: Sleepers = Sleepers::new();

thread_local! {
    static CURRENT: Cell<Option<Serving>> = const { Cell::new(None) };
}

#[derive(Clone, Copy)]
struct Serving {
    id: u64,
    marks: *const Marks,
}

impl Serving {
    fn is(self, handle: &NodeHandle) -> bool {
        self.id == handle.id && std::ptr::eq(self.marks, Arc::as_ptr(&handle.marks))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Repaint,
    Rebuild,
    Relayout,
}

#[derive(Debug, Default)]
pub(crate) struct Marks {
    queue: Mutex<Vec<(u64, Mark)>>,
    waiting: AtomicBool,
}

impl Marks {
    fn push(&self, id: u64, mark: Mark) {
        self.lock().push((id, mark));
        if !self.waiting.swap(true, Ordering::AcqRel) {
            MARKED.wake_all();
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.waiting.load(Ordering::Acquire)
    }

    pub(crate) fn take(&self) -> Vec<(u64, Mark)> {
        if !self.waiting.swap(false, Ordering::AcqRel) {
            return Vec::new();
        }
        std::mem::take(&mut *self.lock())
    }

    fn lock(&self) -> MutexGuard<'_, Vec<(u64, Mark)>> {
        self.queue.lock()
    }
}

pub(crate) fn on_marks(wake: &Waking) {
    MARKED.register(wake);
}

#[derive(Clone)]
pub struct NodeHandle {
    id: u64,
    marks: Arc<Marks>,
}

impl NodeHandle {
    pub(crate) fn new(id: u64, marks: &Arc<Marks>) -> Self {
        NodeHandle {
            id,
            marks: Arc::clone(marks),
        }
    }

    #[must_use]
    pub fn current() -> NodeHandle {
        Self::try_current().expect("NodeHandle::current called outside build or paint")
    }

    pub(crate) fn try_current() -> Option<NodeHandle> {
        let serving = CURRENT.get()?;
        let marks = unsafe {
            Arc::increment_strong_count(serving.marks);
            Arc::from_raw(serving.marks)
        };
        Some(NodeHandle {
            id: serving.id,
            marks,
        })
    }

    pub fn repaint(&self) {
        self.marks.push(self.id, Mark::Repaint);
    }

    pub fn rebuild(&self) {
        self.marks.push(self.id, Mark::Rebuild);
    }

    pub fn relayout(&self) {
        self.marks.push(self.id, Mark::Relayout);
    }

    fn is(&self, other: &NodeHandle) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.marks, &other.marks)
    }
}

impl std::fmt::Debug for NodeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeHandle").field("id", &self.id).finish()
    }
}

pub(crate) fn serving<R>(id: u64, marks: &Arc<Marks>, body: impl FnOnce() -> R) -> R {
    let _serving = CurrentHandleGuard(CURRENT.replace(Some(Serving {
        id,
        marks: Arc::as_ptr(marks),
    })));
    body()
}

struct CurrentHandleGuard(Option<Serving>);

impl Drop for CurrentHandleGuard {
    fn drop(&mut self) {
        CURRENT.set(self.0);
    }
}

#[derive(Clone, Default)]
pub struct Watchers(Arc<Mutex<Vec<NodeHandle>>>);

impl Watchers {
    #[must_use]
    pub fn new() -> Watchers {
        Watchers::default()
    }

    pub fn watch(&self, handle: NodeHandle) {
        let mut watchers = self.lock();
        if !watchers.iter().any(|watched| watched.is(&handle)) {
            watchers.push(handle);
        }
    }

    pub fn watch_current(&self) {
        if let Some(handle) = NodeHandle::try_current() {
            self.watch(handle);
        }
    }

    pub fn repaint(&self) {
        for handle in std::mem::take(&mut *self.lock()) {
            handle.repaint();
        }
    }

    pub fn rebuild(&self) {
        for handle in std::mem::take(&mut *self.lock()) {
            handle.rebuild();
        }
    }

    pub(crate) fn repaint_others(&self) {
        self.notify_others(Mark::Repaint);
    }

    pub(crate) fn relayout_others(&self) {
        self.notify_others(Mark::Relayout);
    }

    fn notify_others(&self, mark: Mark) {
        let current = CURRENT.get();
        let mut watchers = self.lock();
        watchers.retain(|handle| {
            let serving = current.is_some_and(|current| current.is(handle));
            if !serving {
                handle.marks.push(handle.id, mark);
            }
            serving
        });
    }

    fn lock(&self) -> MutexGuard<'_, Vec<NodeHandle>> {
        self.0.lock()
    }
}

impl std::fmt::Debug for Watchers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watchers")
            .field("watching", &self.lock().len())
            .finish()
    }
}
