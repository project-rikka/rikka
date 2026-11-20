use std::sync::{self, MutexGuard, PoisonError, WaitTimeoutResult};
use std::time::Duration;

#[derive(Debug, Default)]
pub struct Condvar(sync::Condvar);

impl Condvar {
    pub const fn new() -> Self {
        Self(sync::Condvar::new())
    }

    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        self.0.wait(guard).unwrap_or_else(PoisonError::into_inner)
    }

    pub fn wait_while<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        condition: impl FnMut(&mut T) -> bool,
    ) -> MutexGuard<'a, T> {
        self.0
            .wait_while(guard, condition)
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn wait_timeout<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        timeout: Duration,
    ) -> (MutexGuard<'a, T>, WaitTimeoutResult) {
        self.0
            .wait_timeout(guard, timeout)
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn wait_timeout_while<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        timeout: Duration,
        condition: impl FnMut(&mut T) -> bool,
    ) -> (MutexGuard<'a, T>, WaitTimeoutResult) {
        self.0
            .wait_timeout_while(guard, timeout, condition)
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn notify_one(&self) {
        self.0.notify_one();
    }

    pub fn notify_all(&self) {
        self.0.notify_all();
    }
}
