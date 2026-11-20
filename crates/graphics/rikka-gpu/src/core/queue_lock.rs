use std::sync::Arc;

use ash::prelude::VkResult;
use ash::vk;
use rikka_sync::{Condvar, Mutex};

pub(crate) struct QueueLockShared {
    held: Mutex<bool>,
    released: Condvar,
}

impl QueueLockShared {
    pub(crate) fn new() -> Self {
        Self {
            held: Mutex::new(false),
            released: Condvar::new(),
        }
    }

    pub(crate) fn lock(&self) {
        *self.released.wait_while(self.held.lock(), |held| *held) = true;
    }

    pub(crate) unsafe fn unlock(&self) {
        *self.held.lock() = false;
        self.released.notify_one();
    }

    pub(crate) fn guard(&self) -> QueueLockGuard<'_> {
        self.lock();
        QueueLockGuard { shared: self }
    }
}

impl std::fmt::Debug for QueueLockShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueueLockShared").finish_non_exhaustive()
    }
}

pub struct QueueLockGuard<'a> {
    shared: &'a QueueLockShared,
}

impl Drop for QueueLockGuard<'_> {
    fn drop(&mut self) {
        unsafe { self.shared.unlock() };
    }
}

#[derive(Debug)]
pub struct LockedQueue {
    queue: vk::Queue,
    lock: Arc<QueueLockShared>,
}

impl LockedQueue {
    pub(crate) fn new(queue: vk::Queue) -> Self {
        Self {
            queue,
            lock: Arc::new(QueueLockShared::new()),
        }
    }

    pub fn handle(&self) -> vk::Queue {
        self.queue
    }

    pub(crate) fn lock(&self) -> &Arc<QueueLockShared> {
        &self.lock
    }

    pub fn guard(&self) -> QueueLockGuard<'_> {
        self.lock.guard()
    }

    pub(crate) fn submit(
        &self,
        device: &ash::Device,
        batches: &[vk::SubmitInfo2<'_>],
        fence: vk::Fence,
    ) -> VkResult<()> {
        let _guard = self.lock.guard();
        unsafe { device.queue_submit2(self.queue, batches, fence) }
    }
}

impl<'a> std::fmt::Debug for QueueLockGuard<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueueLockGuard").finish_non_exhaustive()
    }
}
