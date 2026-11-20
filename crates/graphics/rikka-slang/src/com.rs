use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use crate::ffi::{BlobVtbl, UnknownVtbl};

pub(crate) struct Com<V> {
    raw: NonNull<c_void>,
    vtable: PhantomData<*const V>,
}

unsafe impl<V> Send for Com<V> {}

impl<V> Com<V> {
    pub(crate) unsafe fn from_owned(raw: *mut c_void) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self {
            raw,
            vtable: PhantomData,
        })
    }

    pub(crate) fn raw(&self) -> *mut c_void {
        self.raw.as_ptr()
    }

    pub(crate) fn vtable(&self) -> &V {
        unsafe { &**self.raw.as_ptr().cast::<*const V>() }
    }
}

impl<V> Drop for Com<V> {
    fn drop(&mut self) {
        unsafe {
            let unknown = &**self.raw.as_ptr().cast::<*const UnknownVtbl>();
            (unknown.release)(self.raw.as_ptr());
        }
    }
}

pub(crate) type Blob = Com<BlobVtbl>;

impl Blob {
    pub(crate) fn bytes(&self) -> &[u8] {
        let vtable = self.vtable();
        unsafe {
            let size = (vtable.get_buffer_size)(self.raw());
            let data = (vtable.get_buffer_pointer)(self.raw());
            if size == 0 || data.is_null() {
                return &[];
            }
            core::slice::from_raw_parts(data.cast::<u8>(), size)
        }
    }

    pub(crate) fn text(&self) -> String {
        let bytes = self.bytes();
        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).trim_end().to_owned()
    }
}

#[derive(Default)]
pub(crate) struct Diagnostics {
    raw: *mut c_void,
}

impl Diagnostics {
    pub(crate) fn slot(&mut self) -> *mut *mut c_void {
        self.take();
        &mut self.raw
    }

    pub(crate) fn take(&mut self) -> String {
        let raw = core::mem::replace(&mut self.raw, core::ptr::null_mut());
        unsafe { Blob::from_owned(raw) }
            .map(|blob| blob.text())
            .unwrap_or_default()
    }
}

impl Drop for Diagnostics {
    fn drop(&mut self) {
        self.take();
    }
}
