use std::ptr;

use crate::color;
use crate::error::{Error, Result};
use crate::picture::Picture;
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avutil;
use rikka_ffmpeg::{Encoding, Raster, Sws};

pub struct SwConverter {
    sws: Sws,
}

impl Default for SwConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl SwConverter {
    pub fn new() -> Self {
        Self { sws: Sws::new() }
    }

    pub fn rgba(&mut self, frame: &Frame, rgba: &mut Vec<u8>) -> Result<()> {
        self.rgba_within(frame, None, rgba).map(|_| ())
    }

    pub fn rgba_within(
        &mut self,
        frame: &Frame,
        bounds: Option<(u32, u32)>,
        rgba: &mut Vec<u8>,
    ) -> Result<(u32, u32)> {
        let width = frame.width();
        let height = frame.height();
        if width <= 0 || height <= 0 {
            return Err(Error::Unsupported("empty frame".into()));
        }
        let fitted = fitted((width as u32, height as u32), bounds);
        self.rgba_sized(frame, fitted, rgba)?;
        Ok(fitted)
    }

    pub fn rgba_sized(
        &mut self,
        frame: &Frame,
        (out_width, out_height): (u32, u32),
        rgba: &mut Vec<u8>,
    ) -> Result<()> {
        let width = frame.width();
        let height = frame.height();
        if width <= 0 || height <= 0 || out_width == 0 || out_height == 0 {
            return Err(Error::Unsupported("empty frame".into()));
        }
        self.sws.fit(
            Raster {
                width,
                height,
                format: frame.format(),
            },
            Raster {
                width: out_width as i32,
                height: out_height as i32,
                format: avutil::AV_PIX_FMT_RGBA,
            },
        )?;
        let decoded = encoding_of(frame);
        self.sws.describe(
            decoded,
            Encoding {
                matrix: decoded.matrix,
                full_range: true,
            },
        )?;

        rgba.resize(out_width as usize * out_height as usize * 4, 0);
        let data = [
            rgba.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ];
        let linesize = [out_width as i32 * 4, 0, 0, 0];
        unsafe {
            self.sws.scale(
                (
                    frame.data().as_ptr().cast::<*const u8>(),
                    frame.linesize().as_ptr(),
                    height,
                ),
                (data.as_ptr(), linesize.as_ptr(), out_height as i32),
            )?;
        }
        Ok(())
    }

    pub fn fill(&mut self, frame: &mut Frame, picture: Picture<'_>) -> Result<()> {
        let (source_width, source_height) = (picture.width, picture.height);
        let (width, height) = (source_width as usize, source_height as usize);
        let needed = picture.stride * (height.max(1) - 1) + width * 4;
        if width == 0 || height == 0 || picture.stride < width * 4 || picture.bytes.len() < needed {
            return Err(Error::Unsupported(format!(
                "a {width}x{height} picture of stride {} wants {needed} bytes and was given {}",
                picture.stride,
                picture.bytes.len()
            )));
        }
        let target = Raster {
            width: frame.width(),
            height: frame.height(),
            format: frame.format(),
        };
        self.sws.fit(
            Raster {
                width: source_width as i32,
                height: source_height as i32,
                format: picture.layout.pixel_format(),
            },
            target,
        )?;
        let encoded = encoding_of(frame);
        self.sws.describe(
            Encoding {
                matrix: encoded.matrix,
                full_range: true,
            },
            encoded,
        )?;

        let data = [
            picture.bytes.as_ptr(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ];
        let linesize = [picture.stride as i32, 0, 0, 0];
        unsafe {
            self.sws.scale(
                (data.as_ptr(), linesize.as_ptr(), source_height as i32),
                (
                    frame.data().as_ptr(),
                    frame.linesize().as_ptr(),
                    target.height,
                ),
            )?;
        }
        Ok(())
    }
}

fn encoding_of(frame: &Frame) -> Encoding {
    Encoding {
        matrix: color::matrix(frame.colorspace(), frame.height()),
        full_range: frame.color_range() == avutil::AVCOL_RANGE_JPEG
            || matches!(
                frame.format(),
                avutil::AV_PIX_FMT_YUVJ420P
                    | avutil::AV_PIX_FMT_YUVJ422P
                    | avutil::AV_PIX_FMT_YUVJ444P
            ),
    }
}

pub fn fitted((width, height): (u32, u32), bounds: Option<(u32, u32)>) -> (u32, u32) {
    let Some((most_width, most_height)) = bounds else {
        return (width, height);
    };
    let (most_width, most_height) = (most_width.max(1), most_height.max(1));
    if width <= most_width && height <= most_height {
        return (width, height);
    }
    let scale =
        (f64::from(most_width) / f64::from(width)).min(f64::from(most_height) / f64::from(height));
    let side = |length: u32, most: u32| ((f64::from(length) * scale).round() as u32).clamp(1, most);
    (side(width, most_width), side(height, most_height))
}

impl std::fmt::Debug for SwConverter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SwConverter")
            .finish_non_exhaustive()
    }
}
