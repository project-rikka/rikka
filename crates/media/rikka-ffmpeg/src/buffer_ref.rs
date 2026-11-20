use crate::avutil::{self, AVBufferRef};
use crate::error::{Error, Result};

#[derive(Debug)]
pub struct BufferRef(*mut AVBufferRef);

unsafe impl Send for BufferRef {}

impl BufferRef {
    pub fn from_raw(buffer: *mut AVBufferRef, made_by: &'static str) -> Result<Self> {
        match buffer.is_null() {
            true => Err(Error::Unsupported(format!("{made_by} failed"))),
            false => Ok(Self(buffer)),
        }
    }

    pub fn share(&self) -> Result<Self> {
        Self::from_raw(unsafe { avutil::av_buffer_ref(self.0) }, "av_buffer_ref")
    }

    pub fn as_ptr(&self) -> *mut AVBufferRef {
        self.0
    }

    pub fn into_raw(self) -> *mut AVBufferRef {
        let buffer = self.0;
        std::mem::forget(self);
        buffer
    }
}

impl Drop for BufferRef {
    fn drop(&mut self) {
        unsafe { avutil::av_buffer_unref(&mut self.0) };
    }
}
