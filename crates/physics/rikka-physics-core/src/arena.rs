use std::alloc::{self, Layout};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::mem::MaybeUninit;
use std::ptr::NonNull;

const CHUNK_ALIGN: usize = 64;
const MIN_CHUNK: usize = 64 * 1024;

struct Chunk {
    base: NonNull<u8>,
    size: usize,
}

impl Chunk {
    fn new(size: usize) -> Self {
        let layout = Self::layout(size);
        let base = NonNull::new(unsafe { alloc::alloc(layout) })
            .unwrap_or_else(|| alloc::handle_alloc_error(layout));
        Self { base, size }
    }

    fn layout(size: usize) -> Layout {
        match Layout::from_size_align(size, CHUNK_ALIGN) {
            Ok(layout) => layout,
            Err(error) => panic!("arena chunk of {size} bytes: {error}"),
        }
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        unsafe { alloc::dealloc(self.base.as_ptr(), Self::layout(self.size)) }
    }
}

pub struct Arena {
    current: Cell<Option<NonNull<u8>>>,
    size: Cell<usize>,
    offset: Cell<usize>,
    chunks: RefCell<Vec<Chunk>>,
    retired_bytes: Cell<usize>,
    peak: usize,
}

unsafe impl Send for Arena {}

impl fmt::Debug for Arena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Arena")
            .field("used", &self.used())
            .field("capacity", &self.capacity())
            .finish()
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::mut_from_ref)]
impl Arena {
    pub const fn new() -> Self {
        Self {
            current: Cell::new(None),
            size: Cell::new(0),
            offset: Cell::new(0),
            chunks: RefCell::new(Vec::new()),
            retired_bytes: Cell::new(0),
            peak: 0,
        }
    }

    pub fn with_capacity(bytes: usize) -> Self {
        let arena = Self::new();
        if bytes > 0 {
            arena.install(Chunk::new(bytes.next_multiple_of(CHUNK_ALIGN)));
        }
        arena
    }

    #[inline]
    pub fn used(&self) -> usize {
        self.retired_bytes.get() + self.offset.get()
    }

    #[inline]
    pub fn peak(&self) -> usize {
        self.peak.max(self.used())
    }

    pub fn capacity(&self) -> usize {
        self.chunks.borrow().iter().map(|chunk| chunk.size).sum()
    }

    pub fn reset(&mut self) {
        self.peak = self.peak();
        let chunks = self.chunks.get_mut();
        if chunks.len() > 1 {
            let total = chunks.iter().map(|chunk| chunk.size).sum();
            chunks.clear();
            self.current.set(None);
            self.install(Chunk::new(total));
        }
        self.offset.set(0);
        self.retired_bytes.set(0);
    }

    #[inline]
    pub fn alloc_uninit<T: Copy>(&self, len: usize) -> &mut [MaybeUninit<T>] {
        let layout = match Layout::array::<T>(len) {
            Ok(layout) => layout,
            Err(error) => panic!("arena slice of {len} items: {error}"),
        };
        let data = if layout.size() == 0 {
            NonNull::<MaybeUninit<T>>::dangling()
        } else {
            self.bump(layout).cast()
        };
        unsafe { std::slice::from_raw_parts_mut(data.as_ptr(), len) }
    }

    #[inline]
    pub fn alloc_slice_fill<T: Copy>(&self, len: usize, value: T) -> &mut [T] {
        let slice = self.alloc_uninit(len);
        slice.fill(MaybeUninit::new(value));
        unsafe { assume_init(slice) }
    }

    #[inline]
    pub fn alloc_slice<T: Copy + Default>(&self, len: usize) -> &mut [T] {
        self.alloc_slice_fill(len, T::default())
    }

    #[inline]
    pub fn alloc_slice_copy<T: Copy>(&self, items: &[T]) -> &mut [T] {
        let slice = self.alloc_uninit(items.len());
        for (target, &item) in slice.iter_mut().zip(items) {
            target.write(item);
        }
        unsafe { assume_init(slice) }
    }

    #[inline]
    fn bump(&self, layout: Layout) -> NonNull<u8> {
        if let Some(base) = self.current.get() {
            let offset = self.offset.get();
            let address = base.as_ptr().addr() + offset;
            let start = offset + (address.next_multiple_of(layout.align()) - address);
            if let Some(end) = start.checked_add(layout.size())
                && end <= self.size.get()
            {
                self.offset.set(end);
                return unsafe { base.add(start) };
            }
        }
        self.grow(layout)
    }

    #[cold]
    fn grow(&self, layout: Layout) -> NonNull<u8> {
        let needed = layout.size() + layout.align().max(CHUNK_ALIGN);
        let size = needed
            .max(self.capacity() * 2)
            .max(MIN_CHUNK)
            .next_multiple_of(CHUNK_ALIGN);
        self.retired_bytes
            .set(self.retired_bytes.get() + self.offset.get());
        self.install(Chunk::new(size));
        self.bump(layout)
    }

    fn install(&self, chunk: Chunk) {
        self.current.set(Some(chunk.base));
        self.size.set(chunk.size);
        self.offset.set(0);
        self.chunks.borrow_mut().push(chunk);
    }
}

unsafe fn assume_init<T>(slice: &mut [MaybeUninit<T>]) -> &mut [T] {
    unsafe { &mut *(std::ptr::from_mut(slice) as *mut [T]) }
}
