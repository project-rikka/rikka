use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::CachePadded;

struct Slot<T> {
    borrowed: AtomicBool,
    value: UnsafeCell<T>,
}

pub struct PerWorker<T> {
    slots: Box<[CachePadded<Slot<T>>]>,
}

unsafe impl<T: Send> Sync for PerWorker<T> {}

impl<T: fmt::Debug> fmt::Debug for PerWorker<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        for slot in &self.slots {
            match slot.try_borrow() {
                Some(value) => list.entry(&*value),
                None => list.entry(&format_args!("<in use>")),
            };
        }
        list.finish()
    }
}

impl<T> PerWorker<T> {
    pub fn new(workers: usize, mut value: impl FnMut(usize) -> T) -> Self {
        Self {
            slots: (0..workers)
                .map(|worker| {
                    CachePadded::new(Slot {
                        borrowed: AtomicBool::new(false),
                        value: UnsafeCell::new(value(worker)),
                    })
                })
                .collect(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    #[inline]
    pub fn local(&self, worker: u32) -> WorkerLocal<'_, T> {
        match self.slots[worker as usize].try_borrow() {
            Some(local) => local,
            None => panic!("worker {worker} slot already taken"),
        }
    }

    #[inline]
    pub fn get_mut(&mut self, worker: u32) -> &mut T {
        self.slots[worker as usize].value.get_mut()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut().map(|slot| slot.value.get_mut())
    }

    pub fn into_vec(self) -> Vec<T> {
        self.slots
            .into_iter()
            .map(|slot| slot.into_inner().value.into_inner())
            .collect()
    }
}

impl<T> Slot<T> {
    #[inline]
    fn try_borrow(&self) -> Option<WorkerLocal<'_, T>> {
        if self.borrowed.swap(true, Ordering::Acquire) {
            return None;
        }
        Some(WorkerLocal { slot: self })
    }
}

pub struct WorkerLocal<'a, T> {
    slot: &'a Slot<T>,
}

impl<T: fmt::Debug> fmt::Debug for WorkerLocal<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T> Deref for WorkerLocal<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.slot.value.get() }
    }
}

impl<T> DerefMut for WorkerLocal<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.slot.value.get() }
    }
}

impl<T> Drop for WorkerLocal<'_, T> {
    #[inline]
    fn drop(&mut self) {
        self.slot.borrowed.store(false, Ordering::Release);
    }
}
