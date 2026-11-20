use std::ffi::CStr;
use std::path::Path;
use std::ptr;

use crate::avformat::{self, AVFormatContext};
use crate::error::{Error, Result, check};
use crate::io::{ByteSource, CustomIo};
use crate::path::c_path;

const ARRIVING_PROBE_PACKETS: core::ffi::c_int = 1;

pub struct InputContext {
    ctx: *mut AVFormatContext,
    io: Option<CustomIo>,
}

unsafe impl Send for InputContext {}

impl InputContext {
    pub fn open(path: &Path) -> Result<Self> {
        let path = c_path(path)?;
        let mut ctx: *mut AVFormatContext = ptr::null_mut();
        let status = unsafe {
            avformat::avformat_open_input(&mut ctx, path.as_ptr(), ptr::null(), ptr::null_mut())
        };
        check("avformat_open_input", status)?;
        Ok(Self { ctx, io: None })
    }

    pub fn open_source(source: Box<dyn ByteSource>, name: Option<&CStr>) -> Result<Self> {
        let arriving = source.byte_len().is_none();
        let io = CustomIo::new(source)?;
        let mut ctx = unsafe { avformat::avformat_alloc_context() };
        if ctx.is_null() {
            return Err(Error::Unsupported("avformat_alloc_context failed".into()));
        }
        unsafe {
            (*ctx).pb = io.as_ptr();
            (*ctx).flags |= avformat::AVFMT_FLAG_CUSTOM_IO;
            if arriving {
                (*ctx).max_probe_packets = ARRIVING_PROBE_PACKETS;
                (*ctx).max_ts_probe = ARRIVING_PROBE_PACKETS;
            }
        }
        let name = name.map_or(ptr::null(), CStr::as_ptr);
        let status =
            unsafe { avformat::avformat_open_input(&mut ctx, name, ptr::null(), ptr::null_mut()) };
        check("avformat_open_input", status)?;
        Ok(Self { ctx, io: Some(io) })
    }

    pub fn as_ptr(&self) -> *mut AVFormatContext {
        self.ctx
    }
}

impl std::fmt::Debug for InputContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputContext")
            .field("ctx", &self.ctx)
            .field("custom_io", &self.io.is_some())
            .finish()
    }
}

impl Drop for InputContext {
    fn drop(&mut self) {
        unsafe { avformat::avformat_close_input(&mut self.ctx) };
    }
}
