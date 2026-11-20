use std::rc::Rc;
use std::sync::Arc;
use std::thread::ThreadId;

use crate::abi::violation;

fn present<T>(handle: *mut T, param: &str) -> *mut T {
    if handle.is_null() {
        violation(format!("`{param}` is NULL"));
    }
    handle
}

pub(crate) unsafe fn owned_mut<'a, T>(handle: *mut T, param: &str) -> &'a mut T {
    unsafe { &mut *present(handle, param) }
}

pub(crate) unsafe fn scoped_mut<'a, T>(handle: *mut T, param: &str) -> &'a mut T {
    unsafe { &mut *present(handle, param) }
}

pub(crate) unsafe fn take_owned<T>(handle: *mut T, param: &str) -> T {
    *unsafe { Box::from_raw(present(handle, param)) }
}

pub(crate) fn give_owned<T>(value: T) -> *mut T {
    Box::into_raw(Box::new(value))
}

pub(crate) unsafe fn free_owned<T>(handle: *mut T) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

pub(crate) unsafe fn shared_ref<'a, T: Send + Sync>(handle: *mut T, param: &str) -> &'a T {
    unsafe { &*present(handle, param) }
}

pub(crate) fn give_shared<T: Send + Sync>(value: Arc<T>) -> *mut T {
    Arc::into_raw(value).cast_mut()
}

pub(crate) unsafe fn ref_shared<T: Send + Sync>(handle: *mut T) -> *mut T {
    if !handle.is_null() {
        unsafe { Arc::increment_strong_count(handle) };
    }
    handle
}

pub(crate) unsafe fn unref_shared<T: Send + Sync>(handle: *mut T) {
    if !handle.is_null() {
        unsafe { Arc::decrement_strong_count(handle) };
    }
}

pub(crate) fn lend<T>(value: &T) -> *mut T {
    std::ptr::from_ref(value).cast_mut()
}

struct Bound<T> {
    thread: ThreadId,
    value: T,
}

fn bound<'a, T>(handle: *mut T, param: &str) -> &'a Bound<T> {
    let bound = unsafe { &*present(handle, param).cast::<Bound<T>>() };
    if bound.thread != std::thread::current().id() {
        violation(format!("`{param}` belongs to another thread"));
    }
    bound
}

pub(crate) unsafe fn local_ref<'a, T>(handle: *mut T, param: &str) -> &'a T {
    &bound(handle, param).value
}

pub(crate) fn give_local<T>(value: T) -> *mut T {
    let bound = Rc::new(Bound {
        thread: std::thread::current().id(),
        value,
    });
    Rc::into_raw(bound).cast_mut().cast::<T>()
}

pub(crate) unsafe fn ref_local<T>(handle: *mut T, param: &str) -> *mut T {
    if !handle.is_null() {
        bound(handle, param);
        unsafe { Rc::increment_strong_count(handle.cast::<Bound<T>>()) };
    }
    handle
}

pub(crate) unsafe fn unref_local<T>(handle: *mut T, param: &str) {
    if !handle.is_null() {
        bound(handle, param);
        unsafe { Rc::decrement_strong_count(handle.cast::<Bound<T>>()) };
    }
}
