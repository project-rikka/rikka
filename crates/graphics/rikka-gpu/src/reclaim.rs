use std::any::Any;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use rikka_sync::{Mutex, MutexGuard};

use crate::identity::{BufferId, TargetId};
use crate::texture::TextureId;

#[derive(Clone, Debug, Default)]
pub struct Reclaimer {
    queue: Arc<Mutex<Vec<ReclaimRequest>>>,
}

pub enum ReclaimRequest {
    Texture(TextureId),
    Target(TargetId),
    Buffer(BufferId),
    Gpu(Box<dyn Any + Send>),
    Routed(Box<dyn Any + Send>),
}

impl std::fmt::Debug for ReclaimRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Texture(id) => f.debug_tuple("Texture").field(id).finish(),
            Self::Target(id) => f.debug_tuple("Target").field(id).finish(),
            Self::Buffer(id) => f.debug_tuple("Buffer").field(id).finish(),
            Self::Gpu(_) => f.write_str("Gpu"),
            Self::Routed(_) => f.write_str("Routed"),
        }
    }
}

impl Reclaimer {
    pub fn texture(&self, id: TextureId) {
        self.enqueue(ReclaimRequest::Texture(id));
    }

    pub fn route<T: Any + Send>(&self, request: T) {
        self.enqueue(ReclaimRequest::Routed(Box::new(request)));
    }

    pub fn defer_drop<T: Any + Send>(&self, value: T) {
        self.enqueue(ReclaimRequest::Gpu(Box::new(value)));
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue().is_empty()
    }

    pub fn same(&self, other: &Reclaimer) -> bool {
        Arc::ptr_eq(&self.queue, &other.queue)
    }

    pub fn take_into(&self, into: &mut Vec<ReclaimRequest>) {
        into.append(&mut self.queue());
    }

    pub fn enqueue(&self, what: ReclaimRequest) {
        self.queue().push(what);
    }

    pub fn defer<T: Any + Send>(&self, value: T) -> Deferred<T> {
        Deferred {
            value: Some(value),
            reclaimer: self.clone(),
        }
    }

    pub fn lease(&self, id: TargetId) -> TargetLease {
        TargetLease {
            id,
            reclaimer: self.clone(),
        }
    }

    fn queue(&self) -> MutexGuard<'_, Vec<ReclaimRequest>> {
        self.queue.lock()
    }
}

pub struct Deferred<T: Any + Send> {
    value: Option<T>,
    reclaimer: Reclaimer,
}

impl<T: Any + Send> Deref for Deferred<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value.as_ref().expect("value is taken only on drop")
    }
}

impl<T: Any + Send> DerefMut for Deferred<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("value is taken only on drop")
    }
}

impl<T: Any + Send> Drop for Deferred<T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.reclaimer.enqueue(ReclaimRequest::Gpu(Box::new(value)));
        }
    }
}

#[derive(Debug)]
pub struct TargetLease {
    pub id: TargetId,
    reclaimer: Reclaimer,
}

impl Drop for TargetLease {
    fn drop(&mut self) {
        self.reclaimer.enqueue(ReclaimRequest::Target(self.id));
    }
}

impl<T: Any + Send> std::fmt::Debug for Deferred<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deferred").finish_non_exhaustive()
    }
}
