use std::fmt;
use std::marker::PhantomData;

pub struct Shared<'a, T> {
    ptr: *mut T,
    len: usize,
    borrow: PhantomData<&'a mut [T]>,
}

unsafe impl<T: Send> Send for Shared<'_, T> {}
unsafe impl<T: Send> Sync for Shared<'_, T> {}

impl<T> Clone for Shared<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Shared<'_, T> {}

impl<T> fmt::Debug for Shared<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Shared({})", self.len)
    }
}

impl<'a, T> Shared<'a, T> {
    #[inline]
    pub fn new(slice: &'a mut [T]) -> Self {
        Self {
            ptr: slice.as_mut_ptr(),
            len: slice.len(),
            borrow: PhantomData,
        }
    }

    #[inline]
    pub unsafe fn from_raw(ptr: *mut T, len: usize) -> Self {
        Self {
            ptr,
            len,
            borrow: PhantomData,
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    #[inline(always)]
    pub unsafe fn read(&self, index: usize) -> T
    where
        T: Copy,
    {
        debug_assert!(index < self.len);
        unsafe { *self.ptr.add(index) }
    }

    #[inline(always)]
    pub unsafe fn write(&self, index: usize, value: T) {
        debug_assert!(index < self.len);
        unsafe { *self.ptr.add(index) = value };
    }

    #[inline(always)]
    pub unsafe fn get(&self, index: usize) -> &'a mut T {
        debug_assert!(index < self.len);
        unsafe { &mut *self.ptr.add(index) }
    }

    #[inline(always)]
    pub unsafe fn at(&self, index: usize) -> &'a T {
        debug_assert!(index < self.len);
        unsafe { &*self.ptr.add(index) }
    }

    #[inline(always)]
    pub unsafe fn slice(&self, start: usize, end: usize) -> &'a mut [T] {
        debug_assert!(start <= end && end <= self.len);
        unsafe { std::slice::from_raw_parts_mut(self.ptr.add(start), end - start) }
    }

    #[inline(always)]
    pub fn prefetch(&self, index: usize, bytes: usize) {
        if index < self.len {
            rikka_simd::prefetch(self.ptr.wrapping_add(index), bytes.min(size_of::<T>()));
        }
    }
}

#[inline(always)]
pub fn prefetch<T>(item: &T, bytes: usize) {
    rikka_simd::prefetch(item, bytes.min(size_of::<T>()));
}
