use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Inner<T> {
    mask: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
}

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T: Send> Sync for Inner<T> {}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        let head = *self.head.get_mut();
        let tail = *self.tail.get_mut();
        let mut index = head;
        while index != tail {
            unsafe { (*self.slots[index & self.mask].get()).assume_init_drop() };
            index = index.wrapping_add(1);
        }
    }
}

pub struct Producer<T> {
    inner: Arc<Inner<T>>,
}

pub struct Consumer<T> {
    inner: Arc<Inner<T>>,
}

pub fn channel<T>(min_capacity: usize) -> (Producer<T>, Consumer<T>) {
    let capacity = min_capacity.max(2).next_power_of_two();
    let slots = (0..capacity)
        .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
        .collect();
    let inner = Arc::new(Inner {
        mask: capacity - 1,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        slots,
    });
    (
        Producer {
            inner: inner.clone(),
        },
        Consumer { inner },
    )
}

pub struct Reservation<'a, T> {
    inner: &'a Inner<T>,
    tail: usize,
}

impl<T> Reservation<'_, T> {
    pub fn put(self, value: T) {
        unsafe { (*self.inner.slots[self.tail & self.inner.mask].get()).write(value) };
        self.inner
            .tail
            .store(self.tail.wrapping_add(1), Ordering::Release);
    }
}

impl<T> Producer<T> {
    pub fn reserve(&mut self) -> Option<Reservation<'_, T>> {
        let inner = &*self.inner;
        let tail = inner.tail.load(Ordering::Relaxed);
        let head = inner.head.load(Ordering::Acquire);
        if tail.wrapping_sub(head) > inner.mask {
            return None;
        }
        Some(Reservation { inner, tail })
    }

    pub fn push(&mut self, value: T) -> Result<(), T> {
        match self.reserve() {
            Some(reservation) => {
                reservation.put(value);
                Ok(())
            }
            None => Err(value),
        }
    }

    pub fn len(&self) -> usize {
        let inner = &*self.inner;
        let tail = inner.tail.load(Ordering::Relaxed);
        let head = inner.head.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn free(&self) -> usize {
        self.inner.mask + 1 - self.len()
    }

    pub fn position(&self) -> u64 {
        self.inner.tail.load(Ordering::Relaxed) as u64
    }
}

impl<T: Copy> Producer<T> {
    pub fn push_slice(&mut self, values: &[T]) -> usize {
        let inner = &*self.inner;
        let tail = inner.tail.load(Ordering::Relaxed);
        let head = inner.head.load(Ordering::Acquire);
        let free = inner.mask + 1 - tail.wrapping_sub(head);
        let count = values.len().min(free);
        let start = tail & inner.mask;
        let first = count.min(inner.mask + 1 - start);
        unsafe {
            std::ptr::copy_nonoverlapping(
                values.as_ptr(),
                inner.slots[start].get().cast::<T>(),
                first,
            );
            if first < count {
                std::ptr::copy_nonoverlapping(
                    values.as_ptr().add(first),
                    inner.slots[0].get().cast::<T>(),
                    count - first,
                );
            }
        }
        inner
            .tail
            .store(tail.wrapping_add(count), Ordering::Release);
        count
    }
}

impl<T> Consumer<T> {
    pub fn pop(&mut self) -> Option<T> {
        let inner = &*self.inner;
        let head = inner.head.load(Ordering::Relaxed);
        let tail = inner.tail.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        let value = unsafe { (*inner.slots[head & inner.mask].get()).assume_init_read() };
        inner.head.store(head.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    pub fn len(&self) -> usize {
        let inner = &*self.inner;
        let head = inner.head.load(Ordering::Relaxed);
        let tail = inner.tail.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn free(&self) -> usize {
        self.inner.mask + 1 - self.len()
    }

    pub fn position(&self) -> u64 {
        self.inner.head.load(Ordering::Relaxed) as u64
    }
}

impl<T: Copy> Consumer<T> {
    pub fn pop_slice(&mut self, values: &mut [T]) -> usize {
        self.pop_slice_before(values, u64::MAX)
    }

    pub fn pop_slice_before(&mut self, values: &mut [T], end: u64) -> usize {
        let inner = &*self.inner;
        let head = inner.head.load(Ordering::Relaxed);
        let tail = inner.tail.load(Ordering::Acquire);
        let before_end = usize::try_from(end.saturating_sub(head as u64)).unwrap_or(usize::MAX);
        let available = tail.wrapping_sub(head).min(before_end);
        let count = values.len().min(available);
        let start = head & inner.mask;
        let first = count.min(inner.mask + 1 - start);
        unsafe {
            std::ptr::copy_nonoverlapping(
                inner.slots[start].get().cast::<T>(),
                values.as_mut_ptr(),
                first,
            );
            if first < count {
                std::ptr::copy_nonoverlapping(
                    inner.slots[0].get().cast::<T>(),
                    values.as_mut_ptr().add(first),
                    count - first,
                );
            }
        }
        inner
            .head
            .store(head.wrapping_add(count), Ordering::Release);
        count
    }

    pub fn clear(&mut self) {
        let inner = &*self.inner;
        let tail = inner.tail.load(Ordering::Acquire);
        inner.head.store(tail, Ordering::Release);
    }

    pub fn skip_to(&mut self, position: u64) {
        let inner = &*self.inner;
        let head = inner.head.load(Ordering::Relaxed);
        let tail = inner.tail.load(Ordering::Acquire);
        let target = usize::try_from(position).unwrap_or(usize::MAX);
        if target > head {
            inner.head.store(target.min(tail), Ordering::Release);
        }
    }
}

impl<T> std::fmt::Debug for Producer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Producer")
            .field("len", &self.len())
            .field("free", &self.free())
            .finish()
    }
}

impl<T> std::fmt::Debug for Consumer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Consumer")
            .field("len", &self.len())
            .field("free", &self.free())
            .finish()
    }
}

impl<T> std::fmt::Debug for Reservation<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reservation")
            .field("tail", &self.tail)
            .finish()
    }
}
