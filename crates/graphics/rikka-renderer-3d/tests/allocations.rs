use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

thread_local! {
    static COUNTED: Cell<Option<u64>> = const { Cell::new(None) };
}

fn count() {
    if let Some(count) = COUNTED.try_with(Cell::get).ok().flatten() {
        COUNTED.set(Some(count + 1));
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

pub(crate) fn allocations<T>(run: impl FnOnce() -> T) -> (T, u64) {
    COUNTED.set(Some(0));
    let result = run();
    let counted = COUNTED.take().expect("allocations were counted");
    (result, counted)
}
