use core::ffi::c_int;
use std::ptr;

use rikka_ffmpeg::avutil::{self, AVColorSpace, AVPixelFormat};
use rikka_ffmpeg::{Encoding, Frame, Raster, Sws};

use crate::error::{Error, Result};
use crate::format::Format;
use crate::layout::{Depth, Layout, Picture};

const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];
const WIDE_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Signal {
    pub(crate) matrix: AVColorSpace,
    pub(crate) full_range: bool,
}

impl Signal {
    pub(crate) fn of(frame: &Frame, unmarked_full: bool) -> Signal {
        let full_range = match frame.color_range() {
            avutil::AVCOL_RANGE_JPEG => true,
            avutil::AVCOL_RANGE_MPEG => false,
            _ => unmarked_full,
        };
        Signal {
            matrix: frame.colorspace(),
            full_range,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Region {
    pub(crate) fn whole(picture: &Picture) -> Region {
        Region {
            left: 0,
            top: 0,
            width: picture.width,
            height: picture.height,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Converter {
    format: Format,
    sws: Sws,
    scratch: Vec<u8>,
}

impl Converter {
    pub(crate) fn new(format: Format) -> Converter {
        Converter {
            format,
            sws: Sws::exact(),
            scratch: Vec::new(),
        }
    }

    pub(crate) fn store(
        &mut self,
        frame: &Frame,
        signal: Signal,
        target: &mut Picture,
        region: Region,
    ) -> Result<()> {
        let (width, height) = self.dimensions(frame)?;
        if width < region.width || height < region.height {
            return Err(self.damaged("picture is smaller than declared"));
        }
        let layout = target.layout;
        let bytes_per_pixel = layout.bytes_per_pixel();
        if (width, height) == (target.width, target.height) && region == Region::whole(target) {
            let stride = target.stride();
            return self.convert(frame, signal, layout, target.samples.as_bytes_mut(), stride);
        }
        let stride = width as usize * bytes_per_pixel;
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        scratch.resize(stride * height as usize, 0);
        let converted = self.convert(frame, signal, layout, &mut scratch, stride);
        if converted.is_ok() {
            let target_stride = target.stride();
            let row_bytes = region.width as usize * bytes_per_pixel;
            let bytes = target.samples.as_bytes_mut();
            for row in 0..region.height as usize {
                let from = row * stride;
                let to = (region.top as usize + row) * target_stride
                    + region.left as usize * bytes_per_pixel;
                bytes[to..to + row_bytes].copy_from_slice(&scratch[from..from + row_bytes]);
            }
        }
        self.scratch = scratch;
        converted
    }

    fn dimensions(&self, frame: &Frame) -> Result<(u32, u32)> {
        match (u32::try_from(frame.width()), u32::try_from(frame.height())) {
            (Ok(width), Ok(height)) if width > 0 && height > 0 => Ok((width, height)),
            _ => Err(self.damaged("picture has no size")),
        }
    }

    fn convert(
        &mut self,
        frame: &Frame,
        signal: Signal,
        layout: Layout,
        out: &mut [u8],
        stride: usize,
    ) -> Result<()> {
        let (width, height) = self.dimensions(frame)?;
        if let Some(source) = FloatSource::of(frame.format()) {
            source.store(frame, (width, height), layout, out, stride);
            return Ok(());
        }
        match layout.depth() {
            Depth::Float => {
                let wide_stride = width as usize * WIDE_BYTES;
                let mut wide = vec![0u8; wide_stride * height as usize];
                self.scale(
                    frame,
                    signal,
                    avutil::AV_PIX_FMT_RGBA64LE,
                    &mut wide,
                    wide_stride,
                )?;
                widen(&wide, (width, height), layout, out, stride);
                Ok(())
            }
            Depth::Eight | Depth::Sixteen => {
                self.scale(frame, signal, pixel_format(layout), out, stride)
            }
        }
    }

    fn scale(
        &mut self,
        frame: &Frame,
        signal: Signal,
        to: AVPixelFormat,
        out: &mut [u8],
        stride: usize,
    ) -> Result<()> {
        let (width, height) = (frame.width(), frame.height());
        let failed = |error| Error::decoding(self.format, error);
        self.sws
            .fit(
                Raster {
                    width,
                    height,
                    format: frame.format(),
                },
                Raster {
                    width,
                    height,
                    format: to,
                },
            )
            .map_err(failed)?;
        let read = Encoding {
            matrix: signal.matrix,
            full_range: signal.full_range,
        };
        let written = Encoding {
            matrix: signal.matrix,
            full_range: true,
        };
        self.sws.describe(read, written).map_err(failed)?;
        let planes = [
            out.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        ];
        let lines = [stride as c_int, 0, 0, 0];
        unsafe {
            self.sws.scale(
                (
                    frame.data().as_ptr().cast(),
                    frame.linesize().as_ptr(),
                    height,
                ),
                (planes.as_ptr(), lines.as_ptr(), height),
            )
        }
        .map_err(failed)
    }

    fn damaged(&self, problem: &str) -> Error {
        Error::Decoding {
            format: self.format,
            message: problem.to_owned(),
        }
    }
}

pub(crate) const fn pixel_format(layout: Layout) -> AVPixelFormat {
    match layout {
        Layout::L8 => avutil::AV_PIX_FMT_GRAY8,
        Layout::La8 => avutil::AV_PIX_FMT_YA8,
        Layout::Rgb8 => avutil::AV_PIX_FMT_RGB24,
        Layout::Rgba8 => avutil::AV_PIX_FMT_RGBA,
        Layout::L16 => avutil::AV_PIX_FMT_GRAY16LE,
        Layout::La16 => avutil::AV_PIX_FMT_YA16LE,
        Layout::Rgb16 => avutil::AV_PIX_FMT_RGB48LE,
        Layout::Rgba16 | Layout::Rgb32F | Layout::Rgba32F => avutil::AV_PIX_FMT_RGBA64LE,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channels {
    Gray,
    GrayAlpha,
    Rgb,
    Rgba,
}

#[derive(Clone, Copy, Debug)]
struct FloatSource {
    channels: Channels,
    planar: bool,
    half: bool,
    big_endian: bool,
}

impl FloatSource {
    const fn of(format: AVPixelFormat) -> Option<FloatSource> {
        let (channels, planar, half, big_endian) = match format {
            avutil::AV_PIX_FMT_GBRPF32LE => (Channels::Rgb, true, false, false),
            avutil::AV_PIX_FMT_GBRPF32BE => (Channels::Rgb, true, false, true),
            avutil::AV_PIX_FMT_GBRAPF32LE => (Channels::Rgba, true, false, false),
            avutil::AV_PIX_FMT_GBRAPF32BE => (Channels::Rgba, true, false, true),
            avutil::AV_PIX_FMT_GRAYF32LE => (Channels::Gray, true, false, false),
            avutil::AV_PIX_FMT_YAF32LE => (Channels::GrayAlpha, false, false, false),
            avutil::AV_PIX_FMT_RGBF32LE => (Channels::Rgb, false, false, false),
            avutil::AV_PIX_FMT_RGBF32BE => (Channels::Rgb, false, false, true),
            avutil::AV_PIX_FMT_RGBAF32LE => (Channels::Rgba, false, false, false),
            avutil::AV_PIX_FMT_RGBAF32BE => (Channels::Rgba, false, false, true),
            avutil::AV_PIX_FMT_GBRPF16LE => (Channels::Rgb, true, true, false),
            avutil::AV_PIX_FMT_GBRAPF16LE => (Channels::Rgba, true, true, false),
            avutil::AV_PIX_FMT_GRAYF16LE => (Channels::Gray, true, true, false),
            avutil::AV_PIX_FMT_YAF16LE => (Channels::GrayAlpha, false, true, false),
            _ => return None,
        };
        Some(FloatSource {
            channels,
            planar,
            half,
            big_endian,
        })
    }

    fn sample_bytes(self) -> usize {
        match self.half {
            true => 2,
            false => 4,
        }
    }

    fn read(self, bytes: &[u8]) -> f32 {
        match (self.half, self.big_endian) {
            (true, false) => half(u16::from_le_bytes([bytes[0], bytes[1]])),
            (true, true) => half(u16::from_be_bytes([bytes[0], bytes[1]])),
            (false, false) => f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            (false, true) => f32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }

    fn store(
        self,
        frame: &Frame,
        (width, height): (u32, u32),
        layout: Layout,
        out: &mut [u8],
        stride: usize,
    ) {
        let size = self.sample_bytes();
        let (count, plane_of): (usize, &[usize]) = match self.channels {
            Channels::Gray => (1, &[0]),
            Channels::GrayAlpha => (2, &[0, 0]),
            Channels::Rgb => (3, &[2, 0, 1]),
            Channels::Rgba => (4, &[2, 0, 1, 3]),
        };
        let data = frame.data();
        let lines = frame.linesize();
        let width = width as usize;
        for y in 0..height as usize {
            let row = |plane: usize, length: usize| unsafe {
                std::slice::from_raw_parts(
                    data[plane].add(y * lines[plane] as usize).cast_const(),
                    length,
                )
            };
            let out_row = &mut out[y * stride..y * stride + width * layout.bytes_per_pixel()];
            for x in 0..width {
                let mut values = [0.0f32; 4];
                for (channel, value) in values.iter_mut().enumerate().take(count) {
                    let at = match self.planar {
                        true => x * size,
                        false => (x * count + channel) * size,
                    };
                    let plane = match self.planar {
                        true => plane_of[channel],
                        false => 0,
                    };
                    let length = match self.planar {
                        true => width * size,
                        false => width * count * size,
                    };
                    *value = self.read(&row(plane, length)[at..at + size]);
                }
                let rgba = match self.channels {
                    Channels::Gray => [values[0], values[0], values[0], 1.0],
                    Channels::GrayAlpha => [values[0], values[0], values[0], values[1]],
                    Channels::Rgb => [values[0], values[1], values[2], 1.0],
                    Channels::Rgba => values,
                };
                put(layout, out_row, x, rgba);
            }
        }
    }
}

fn widen(wide: &[u8], (width, height): (u32, u32), layout: Layout, out: &mut [u8], stride: usize) {
    let width = width as usize;
    for y in 0..height as usize {
        let source = &wide[y * width * WIDE_BYTES..(y + 1) * width * WIDE_BYTES];
        let out_row = &mut out[y * stride..y * stride + width * layout.bytes_per_pixel()];
        for (x, pixel) in source.as_chunks::<WIDE_BYTES>().0.iter().enumerate() {
            let channel =
                |at: usize| f32::from(u16::from_le_bytes([pixel[at], pixel[at + 1]])) / 65535.0;
            put(
                layout,
                out_row,
                x,
                [channel(0), channel(2), channel(4), channel(6)],
            );
        }
    }
}

fn put(layout: Layout, row: &mut [u8], x: usize, rgba: [f32; 4]) {
    let luma = LUMA[0] * rgba[0] + LUMA[1] * rgba[1] + LUMA[2] * rgba[2];
    let values: &[f32] = match layout {
        Layout::L8 | Layout::L16 => &[luma],
        Layout::La8 | Layout::La16 => &[luma, rgba[3]],
        Layout::Rgb8 | Layout::Rgb16 | Layout::Rgb32F => &rgba[..3],
        Layout::Rgba8 | Layout::Rgba16 | Layout::Rgba32F => &rgba,
    };
    let size = layout.bytes_per_sample();
    let start = x * layout.bytes_per_pixel();
    for (channel, &value) in values.iter().enumerate() {
        let at = start + channel * size;
        match layout.depth() {
            Depth::Eight => row[at] = (value.clamp(0.0, 1.0) * 255.0).round() as u8,
            Depth::Sixteen => {
                let sample = (value.clamp(0.0, 1.0) * 65535.0).round() as u16;
                row[at..at + 2].copy_from_slice(&sample.to_ne_bytes());
            }
            Depth::Float => row[at..at + 4].copy_from_slice(&value.to_ne_bytes()),
        }
    }
}

fn half(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = u32::from((bits >> 10) & 0x1F);
    let mantissa = u32::from(bits & 0x3FF);
    let magnitude = match exponent {
        0 => mantissa as f32 * f32::from_bits(0x3380_0000),
        0x1F if mantissa == 0 => f32::INFINITY,
        0x1F => f32::NAN,
        _ => f32::from_bits(((exponent + 112) << 23) | (mantissa << 13)),
    };
    f32::from_bits(magnitude.to_bits() | sign)
}
