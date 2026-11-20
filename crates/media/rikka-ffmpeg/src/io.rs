use core::ffi::{c_int, c_void};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::avformat::{self, AVIOContext};
use crate::avutil;
use crate::error::{Error, Result};

const BUFFER_SIZE: usize = 64 * 1024;
const SEEK_SET: c_int = 0;
const SEEK_CUR: c_int = 1;
const SEEK_END: c_int = 2;

type Seeking = unsafe extern "C" fn(*mut c_void, i64, c_int) -> i64;

pub trait ByteSource: Read + Seek + Send {
    fn byte_len(&self) -> Option<u64>;
}

pub(crate) struct CustomIo {
    ctx: *mut AVIOContext,
    source: *mut Box<dyn ByteSource>,
}

unsafe impl Send for CustomIo {}

impl CustomIo {
    pub(crate) fn new(source: Box<dyn ByteSource>) -> Result<Self> {
        let buffer = unsafe { avutil::av_malloc(BUFFER_SIZE) }.cast::<u8>();
        if buffer.is_null() {
            return Err(Error::Unsupported("av_malloc failed".into()));
        }
        let seeking = source.byte_len().map(|_| seek as Seeking);
        let source = Box::into_raw(Box::new(source));
        let ctx = unsafe {
            avformat::avio_alloc_context(
                buffer,
                BUFFER_SIZE as c_int,
                0,
                source.cast(),
                Some(read),
                None,
                seeking,
            )
        };
        if ctx.is_null() {
            unsafe {
                avutil::av_free(buffer.cast());
                drop(Box::from_raw(source));
            }
            return Err(Error::Unsupported("avio_alloc_context failed".into()));
        }
        Ok(Self { ctx, source })
    }

    pub(crate) fn as_ptr(&self) -> *mut AVIOContext {
        self.ctx
    }
}

impl Drop for CustomIo {
    fn drop(&mut self) {
        unsafe {
            avutil::av_freep((&raw mut (*self.ctx).buffer).cast());
            avformat::avio_context_free(&mut self.ctx);
            drop(Box::from_raw(self.source));
        }
    }
}

unsafe extern "C" fn read(opaque: *mut c_void, buffer: *mut u8, size: c_int) -> c_int {
    let source = unsafe { &mut *opaque.cast::<Box<dyn ByteSource>>() };
    let Ok(size) = usize::try_from(size) else {
        return avutil::AVERROR_EINVAL;
    };
    let buffer = unsafe { std::slice::from_raw_parts_mut(buffer, size) };
    match catch_unwind(AssertUnwindSafe(|| read_some(source.as_mut(), buffer))) {
        Ok(Ok(0)) => avutil::AVERROR_EOF,
        Ok(Ok(read)) => read as c_int,
        Ok(Err(_)) | Err(_) => avutil::AVERROR_EIO,
    }
}

unsafe extern "C" fn seek(opaque: *mut c_void, offset: i64, whence: c_int) -> i64 {
    let source = unsafe { &mut *opaque.cast::<Box<dyn ByteSource>>() };
    let moved = catch_unwind(AssertUnwindSafe(|| {
        if whence & avformat::AVSEEK_SIZE != 0 {
            return source
                .byte_len()
                .and_then(|length| i64::try_from(length).ok());
        }
        let to = match whence & !avformat::AVSEEK_FORCE {
            SEEK_SET => SeekFrom::Start(u64::try_from(offset).ok()?),
            SEEK_CUR => SeekFrom::Current(offset),
            SEEK_END => SeekFrom::End(offset),
            _ => return None,
        };
        source
            .seek(to)
            .ok()
            .and_then(|position| i64::try_from(position).ok())
    }));
    match moved {
        Ok(Some(position)) => position,
        Ok(None) | Err(_) => i64::from(avutil::AVERROR_EIO),
    }
}

fn read_some(source: &mut dyn ByteSource, buffer: &mut [u8]) -> std::io::Result<usize> {
    loop {
        match source.read(buffer) {
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            other => return other,
        }
    }
}
