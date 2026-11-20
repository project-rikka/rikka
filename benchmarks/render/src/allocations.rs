use rikka_sync::Mutex;
use std::alloc::{GlobalAlloc, Layout, System};
use std::backtrace::Backtrace;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const TRACED: usize = 8;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

static TRACING: AtomicBool = AtomicBool::new(false);

static TRACES: Mutex<Vec<String>> = Mutex::new(Vec::new());

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(true) };
}

#[derive(Debug)]
pub struct Counting;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Warming,
    Steady,
}

fn count() {
    if !MEASURING.with(Cell::get) {
        return;
    }
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    if TRACING.load(Ordering::Relaxed) {
        uncounted(trace);
    }
}

fn trace() {
    let mut traces = TRACES.lock();
    if traces.len() < TRACED {
        let thread = std::thread::current();
        traces.push(format!(
            "allocation on thread {}:\n{}",
            thread.name().unwrap_or("unnamed"),
            Backtrace::force_capture()
        ));
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(pointer, layout, size) }
    }
}

pub(crate) fn counted<T>(phase: Phase, run: impl FnOnce() -> T) -> (T, u64) {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    TRACING.store(phase == Phase::Steady, Ordering::Relaxed);
    let result = run();
    TRACING.store(false, Ordering::Relaxed);
    (result, ALLOCATIONS.load(Ordering::Relaxed) - before)
}

pub(crate) fn take_traces() -> Vec<String> {
    uncounted(|| std::mem::take(&mut *TRACES.lock()))
}

pub(crate) fn uncounted<T>(run: impl FnOnce() -> T) -> T {
    let measuring = MEASURING.with(|cell| cell.replace(false));
    let result = run();
    MEASURING.with(|cell| cell.set(measuring));
    result
}
