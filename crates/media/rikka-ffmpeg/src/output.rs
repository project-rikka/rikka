use core::ffi::c_int;
use std::ffi::CString;
use std::path::Path;
use std::ptr;

use crate::avcodec;
use crate::avformat::{self, AVFormatContext};
use crate::avutil;
use crate::error::{Result, av_err, check};
use crate::packet::Packet;
use crate::path::c_path;

#[derive(Debug)]
pub struct OutputContext {
    ctx: *mut AVFormatContext,
    path: CString,
}

unsafe impl Send for OutputContext {}

impl OutputContext {
    pub fn create(path: &Path) -> Result<Self> {
        let path = c_path(path)?;
        let mut ctx: *mut AVFormatContext = ptr::null_mut();
        let allocated = unsafe {
            avformat::avformat_alloc_output_context2(
                &mut ctx,
                ptr::null(),
                ptr::null(),
                path.as_ptr(),
            )
        };
        check("avformat_alloc_output_context2", allocated)?;
        Ok(Self { ctx, path })
    }

    pub fn as_ptr(&self) -> *mut AVFormatContext {
        self.ctx
    }

    pub fn wants_global_header(&self) -> bool {
        self.flags() & avformat::AVFMT_GLOBALHEADER != 0
    }

    pub fn is_quicktime(&self) -> bool {
        matches!(self.format_name().to_bytes(), b"mp4" | b"mov" | b"ipod")
    }

    pub fn format_name(&self) -> &std::ffi::CStr {
        let name = unsafe { (*(*self.ctx).oformat).name };
        match name.is_null() {
            true => c"",
            false => unsafe { std::ffi::CStr::from_ptr(name) },
        }
    }

    pub fn accepts(&self, codec: avcodec::AVCodecID) -> bool {
        unsafe { avformat::avformat_query_codec((*self.ctx).oformat, codec, 0) == 1 }
    }

    pub fn write_header(&mut self, fast_start: bool) -> Result<()> {
        if self.has_file_output() {
            let status = unsafe {
                avformat::avio_open(
                    &raw mut (*self.ctx).pb,
                    self.path.as_ptr(),
                    avformat::AVIO_FLAG_WRITE,
                )
            };
            check("avio_open", status)?;
        }
        let mut options = ptr::null_mut();
        if fast_start && self.is_quicktime() {
            let set = unsafe {
                avutil::av_dict_set(
                    &mut options,
                    c"movflags".as_ptr(),
                    c"+faststart".as_ptr(),
                    0,
                )
            };
            check("av_dict_set(movflags)", set)?;
        }
        let status = unsafe { avformat::avformat_write_header(self.ctx, &mut options) };
        unsafe { avutil::av_dict_free(&mut options) };
        check("avformat_write_header", status)?;
        Ok(())
    }

    pub fn write(&mut self, packet: &mut Packet, context: &'static str) -> Result<()> {
        let status = unsafe { avformat::av_interleaved_write_frame(self.ctx, packet.as_mut_ptr()) };
        match status < 0 {
            true => Err(av_err(context, status)),
            false => Ok(()),
        }
    }

    pub fn write_trailer(&mut self) -> Result<()> {
        let status = unsafe { avformat::av_write_trailer(self.ctx) };
        check("av_write_trailer", status)?;
        Ok(())
    }

    fn flags(&self) -> c_int {
        unsafe { (*(*self.ctx).oformat).flags }
    }

    fn has_file_output(&self) -> bool {
        self.flags() & avformat::AVFMT_NOFILE == 0
    }
}

impl Drop for OutputContext {
    fn drop(&mut self) {
        unsafe {
            if self.has_file_output() && !(*self.ctx).pb.is_null() {
                avformat::avio_closep(&raw mut (*self.ctx).pb);
            }
            avformat::avformat_free_context(self.ctx);
        }
    }
}
