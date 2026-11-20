use core::ffi::c_int;
use std::ffi::CStr;
use std::ptr;

use rikka_ffmpeg::avcodec::{self, AVCodec, AVCodecContext};
use rikka_ffmpeg::avutil::{self, AVPixelFormat};
use rikka_ffmpeg::{CodecContext, Frame, Packet, av_err, check};

use crate::error::{Error, Result};
use crate::format::Format;

#[derive(Debug)]
pub(crate) struct Encoder {
    format: Format,
    ctx: CodecContext,
    frame: Frame,
    packet: Packet,
}

pub(crate) struct Opening<'o> {
    pub(crate) name: &'static str,
    pub(crate) codec: *const AVCodec,
    pub(crate) size: (u32, u32),
    pub(crate) pixel_format: AVPixelFormat,
    pub(crate) options: &'o [(&'o CStr, &'o CStr)],
}

impl Encoder {
    pub(crate) fn open(
        format: Format,
        opening: Opening<'_>,
        configure: impl FnOnce(&mut AVCodecContext),
    ) -> Result<Encoder> {
        let failed = |error| Error::encoding(format, error);
        if opening.codec.is_null() {
            return Err(Error::NoEncoder(opening.name));
        }
        let ctx = unsafe { CodecContext::alloc(opening.codec) }.map_err(failed)?;
        let (width, height) = opening.size;
        unsafe {
            let raw = &mut *ctx.as_ptr();
            raw.width = width as c_int;
            raw.height = height as c_int;
            raw.pix_fmt = opening.pixel_format;
            raw.time_base = avutil::AVRational { num: 1, den: 1 };
            raw.framerate = avutil::AVRational { num: 1, den: 1 };
            configure(raw);
            for (name, value) in opening.options {
                let set = avutil::av_opt_set(raw.priv_data, name.as_ptr(), value.as_ptr(), 0);
                check("av_opt_set", set).map_err(failed)?;
            }
            check(
                "avcodec_open2",
                avcodec::avcodec_open2(raw, opening.codec, ptr::null_mut()),
            )
            .map_err(failed)?;
        }
        let mut frame = Frame::new().map_err(failed)?;
        unsafe {
            let raw = &mut *frame.as_mut_ptr();
            raw.width = width as c_int;
            raw.height = height as c_int;
            raw.format = opening.pixel_format;
            check("av_frame_get_buffer", avutil::av_frame_get_buffer(raw, 0)).map_err(failed)?;
        }
        Ok(Encoder {
            format,
            ctx,
            frame,
            packet: Packet::new().map_err(failed)?,
        })
    }

    pub(crate) fn context(&self) -> *mut AVCodecContext {
        self.ctx.as_ptr()
    }

    pub(crate) fn frame(&mut self) -> &mut Frame {
        &mut self.frame
    }

    pub(crate) fn finish(&mut self, mut each: impl FnMut(&mut Packet) -> Result<()>) -> Result<()> {
        let failed = |error| Error::encoding(self.format, error);
        let ctx = self.ctx.as_ptr();
        unsafe {
            check(
                "avcodec_send_frame",
                avcodec::avcodec_send_frame(ctx, self.frame.as_ptr()),
            )
            .map_err(failed)?;
            check(
                "avcodec_send_frame",
                avcodec::avcodec_send_frame(ctx, ptr::null()),
            )
            .map_err(failed)?;
        }
        loop {
            let status = unsafe { avcodec::avcodec_receive_packet(ctx, self.packet.as_mut_ptr()) };
            match status {
                0 => {
                    let written = each(&mut self.packet);
                    self.packet.unref();
                    written?;
                }
                avutil::AVERROR_EOF => return Ok(()),
                code => return Err(failed(av_err("avcodec_receive_packet", code))),
            }
        }
    }
}

pub(crate) fn plane_rows(
    frame: &mut Frame,
    plane: usize,
    height: u32,
    row_bytes: usize,
) -> Vec<&mut [u8]> {
    let data = frame.data()[plane];
    let line = frame.linesize()[plane] as usize;
    (0..height as usize)
        .map(|row| unsafe { std::slice::from_raw_parts_mut(data.add(row * line), row_bytes) })
        .collect()
}
