use core::ffi::c_int;
use std::ptr;

use crate::avutil::{self, AVColorSpace, AVPixelFormat};
use crate::error::{Error, Result};
use crate::swscale::{self, SwsContext};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: i32,
    pub height: i32,
    pub format: AVPixelFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoding {
    pub matrix: AVColorSpace,
    pub full_range: bool,
}

#[derive(Debug)]
pub struct Sws {
    ctx: *mut SwsContext,
    described: Option<(Encoding, Encoding)>,
    filter: c_int,
}

unsafe impl Send for Sws {}

impl Sws {
    pub fn new() -> Self {
        Self::filtered(swscale::SWS_BILINEAR)
    }

    pub fn bicubic() -> Self {
        Self::filtered(swscale::SWS_BICUBIC)
    }

    pub fn exact() -> Self {
        Self::filtered(
            swscale::SWS_BICUBIC
                | swscale::SWS_ACCURATE_RND
                | swscale::SWS_FULL_CHR_H_INT
                | swscale::SWS_FULL_CHR_H_INP,
        )
    }

    fn filtered(filter: c_int) -> Self {
        Self {
            ctx: ptr::null_mut(),
            described: None,
            filter,
        }
    }

    pub fn fit(&mut self, from: Raster, to: Raster) -> Result<()> {
        let cached = unsafe {
            swscale::sws_getCachedContext(
                self.ctx,
                from.width,
                from.height,
                from.format,
                to.width,
                to.height,
                to.format,
                self.filter,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if cached != self.ctx {
            self.described = None;
        }
        self.ctx = cached;
        match self.ctx.is_null() {
            true => Err(Error::Unsupported(format!(
                "no swscale path from {} to {}",
                pixel_format_name(from.format),
                pixel_format_name(to.format)
            ))),
            false => Ok(()),
        }
    }

    pub fn describe(&mut self, from: Encoding, to: Encoding) -> Result<()> {
        if self.described == Some((from, to)) {
            return Ok(());
        }
        let described = unsafe {
            swscale::sws_setColorspaceDetails(
                self.ctx,
                swscale::sws_getCoefficients(from.matrix),
                c_int::from(from.full_range),
                swscale::sws_getCoefficients(to.matrix),
                c_int::from(to.full_range),
                0,
                swscale::SWS_UNITY,
                swscale::SWS_UNITY,
            )
        };
        match described < 0 {
            true => Err(Error::Unsupported(format!(
                "swscale refused the colour matrix {} ({} range) to {} ({} range)",
                from.matrix,
                range_name(from.full_range),
                to.matrix,
                range_name(to.full_range)
            ))),
            false => {
                self.described = Some((from, to));
                Ok(())
            }
        }
    }

    pub unsafe fn scale(
        &self,
        from: (*const *const u8, *const c_int, c_int),
        to: (*const *mut u8, *const c_int, c_int),
    ) -> Result<()> {
        let converted =
            unsafe { swscale::sws_scale(self.ctx, from.0, from.1, 0, from.2, to.0, to.1) };
        match converted == to.2 {
            true => Ok(()),
            false => Err(Error::Unsupported(format!(
                "sws_scale converted {converted} of {} rows",
                to.2
            ))),
        }
    }
}

impl Default for Sws {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Sws {
    fn drop(&mut self) {
        unsafe { swscale::sws_freeContext(self.ctx) };
    }
}

fn range_name(full: bool) -> &'static str {
    match full {
        true => "full",
        false => "limited",
    }
}

pub fn pixel_format_name(format: AVPixelFormat) -> String {
    let name = unsafe { avutil::av_get_pix_fmt_name(format) };
    match name.is_null() {
        true => format!("pixel format {format}"),
        false => unsafe { core::ffi::CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned(),
    }
}
