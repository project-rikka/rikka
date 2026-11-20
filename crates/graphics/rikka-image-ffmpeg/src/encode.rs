use core::ffi::c_int;
use std::ffi::CString;
use std::ptr;

use rikka_ffmpeg::avcodec::{self, AVCodecContext};
use rikka_ffmpeg::avutil::{self, AVPixelFormat};
use rikka_ffmpeg::{Encoding, Raster, Sws};

use crate::av1::Still;
use crate::avif::{Picture, Plane};
use crate::error::{Error, Result};
use crate::ffmpeg::encoder::{Encoder, Opening, plane_rows};
use crate::ffmpeg::store::pixel_format;
use crate::format::Format;
use crate::layout::{Depth, Layout, Pixels};
use crate::tiff;

const AV1_ENCODER: &std::ffi::CStr = c"libaom-av1";
const AV1_CONSTANT_QUALITY_STEPS: u32 = 63;
const AV1_FASTEST_PRESET: u32 = 8;
const TEN_BIT_MAX: u32 = 1023;
const COLOR_CHANNELS: u8 = 3;
const JPEG_XL_ENCODER: &std::ffi::CStr = c"libjxl";
const JPEG_XL_MOST_EFFORT: u32 = 9;
const JPEG_XL_LOSSLESS_QUALITY: f32 = 100.0;
const JPEG_XL_LINEAR_QUALITY: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings<'p> {
    pub quality: u8,
    pub speed: u8,
    pub profile: Option<&'p [u8]>,
}

pub fn encode(format: Format, pixels: Pixels<'_>, settings: Settings<'_>) -> Result<Vec<u8>> {
    pixels.validate()?;
    rikka_ffmpeg::load_scaling().map_err(|error| Error::encoding(format, error))?;
    let profile = settings.profile.filter(|profile| !profile.is_empty());
    match format {
        Format::Avif => avif(pixels, settings, profile),
        Format::Tiff => {
            let file = tiff_file(pixels)?;
            match profile {
                Some(profile) => tiff::with_profile(&file, profile),
                None => Ok(file),
            }
        }
        Format::OpenExr => exr(pixels),
        Format::JpegXl => jpeg_xl(pixels, settings, profile),
        Format::Heif | Format::Jpeg2000 => Err(Error::unsupported(format, "encoding")),
    }
}

fn jpeg_xl(pixels: Pixels<'_>, settings: Settings<'_>, profile: Option<&[u8]>) -> Result<Vec<u8>> {
    let format = Format::JpegXl;
    let speed = u32::from(settings.speed.clamp(1, 10));
    let effort = number(JPEG_XL_MOST_EFFORT - ((speed - 1) * (JPEG_XL_MOST_EFFORT - 1) + 4) / 9);
    let distance = CString::new(jpeg_xl_distance(settings.quality).to_string())
        .unwrap_or_else(|_| unreachable!("numbers hold no NUL"));
    let pixel_format = match pixels.layout {
        Layout::Rgb32F => avutil::AV_PIX_FMT_RGBF32LE,
        Layout::Rgba32F => avutil::AV_PIX_FMT_RGBAF32LE,
        layout => pixel_format(layout),
    };
    let opening = Opening {
        name: "JPEG XL",
        codec: unsafe { avcodec::avcodec_find_encoder_by_name(JPEG_XL_ENCODER.as_ptr()) },
        size: (pixels.width, pixels.height),
        pixel_format,
        options: &[
            (c"distance", distance.as_c_str()),
            (c"effort", effort.as_c_str()),
        ],
    };
    let mut encoder = Encoder::open(format, opening, |ctx| {
        ctx.thread_count = 0;
        ctx.color_primaries = avutil::AVCOL_PRI_BT709;
        ctx.color_trc = avutil::AVCOL_TRC_IEC61966_2_1;
        ctx.color_range = avutil::AVCOL_RANGE_JPEG;
    })?;
    let stride = pixels.stride();
    let rows = plane_rows(encoder.frame(), 0, pixels.height, stride);
    for (row, source) in rows.into_iter().zip(pixels.bytes.chunks_exact(stride)) {
        row.copy_from_slice(source);
    }
    if let Some(profile) = profile {
        encoder
            .frame()
            .set_icc_profile(profile)
            .map_err(|error| Error::encoding(format, error))?;
    }
    whole_file(&mut encoder)
}

fn jpeg_xl_distance(quality: u8) -> f32 {
    let quality = f32::from(quality.clamp(1, 100));
    if quality >= JPEG_XL_LOSSLESS_QUALITY {
        return 0.0;
    }
    if quality >= JPEG_XL_LINEAR_QUALITY {
        return 0.1 + (JPEG_XL_LOSSLESS_QUALITY - quality) * 0.09;
    }
    53.0 / 3000.0 * quality * quality - 23.0 / 20.0 * quality + 25.0
}

fn tiff_file(pixels: Pixels<'_>) -> Result<Vec<u8>> {
    if pixels.layout.depth() == Depth::Float {
        return Err(Error::unsupported(Format::Tiff, "floating-point samples"));
    }
    let opening = Opening {
        name: "TIFF",
        codec: unsafe { avcodec::avcodec_find_encoder(avcodec::AV_CODEC_ID_TIFF) },
        size: (pixels.width, pixels.height),
        pixel_format: pixel_format(pixels.layout),
        options: &[(c"compression_algo", c"lzw")],
    };
    let mut encoder = Encoder::open(Format::Tiff, opening, |_| {})?;
    let stride = pixels.stride();
    let rows = plane_rows(encoder.frame(), 0, pixels.height, stride);
    for (row, source) in rows.into_iter().zip(pixels.bytes.chunks_exact(stride)) {
        row.copy_from_slice(source);
    }
    whole_file(&mut encoder)
}

fn exr(pixels: Pixels<'_>) -> Result<Vec<u8>> {
    let pixel_format = match pixels.layout {
        Layout::Rgb32F => avutil::AV_PIX_FMT_GBRPF32LE,
        Layout::Rgba32F => avutil::AV_PIX_FMT_GBRAPF32LE,
        _ => return Err(Error::unsupported(Format::OpenExr, "integer samples")),
    };
    let opening = Opening {
        name: "OpenEXR",
        codec: unsafe { avcodec::avcodec_find_encoder(avcodec::AV_CODEC_ID_EXR) },
        size: (pixels.width, pixels.height),
        pixel_format,
        options: &[(c"compression", c"zip16"), (c"format", c"float")],
    };
    let mut encoder = Encoder::open(Format::OpenExr, opening, |_| {})?;
    let channels = pixels.layout.channels();
    let row_bytes = pixels.width as usize * 4;
    let planes_of_channels: &[usize] = match channels {
        3 => &[2, 0, 1],
        _ => &[2, 0, 1, 3],
    };
    let sample = |index: usize| {
        let at = index * 4;
        let bytes = &pixels.bytes[at..at + 4];
        f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    };
    let alpha = pixels.layout.has_alpha();
    for (channel, &plane) in planes_of_channels.iter().enumerate() {
        let rows = plane_rows(encoder.frame(), plane, pixels.height, row_bytes);
        for (y, row) in rows.into_iter().enumerate() {
            for (x, out) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let pixel = (y * pixels.width as usize + x) * channels;
                let value = match alpha && channel < 3 {
                    true => sample(pixel + channel) * sample(pixel + 3),
                    false => sample(pixel + channel),
                };
                out.copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    whole_file(&mut encoder)
}

fn whole_file(encoder: &mut Encoder) -> Result<Vec<u8>> {
    let mut file = Vec::new();
    encoder.finish(|packet| {
        file.extend_from_slice(packet.bytes());
        Ok(())
    })?;
    Ok(file)
}

fn avif(pixels: Pixels<'_>, settings: Settings<'_>, profile: Option<&[u8]>) -> Result<Vec<u8>> {
    let format = Format::Avif;
    let deep = match pixels.layout.depth() {
        Depth::Eight => false,
        Depth::Sixteen => true,
        Depth::Float => return Err(Error::unsupported(format, "floating-point samples")),
    };
    let codec = unsafe { avcodec::avcodec_find_encoder_by_name(AV1_ENCODER.as_ptr()) };
    let quality = u32::from(settings.quality.clamp(1, 100));
    let speed = u32::from(settings.speed.clamp(1, 10));
    let crf = number(((100 - quality) * AV1_CONSTANT_QUALITY_STEPS + 50) / 100);
    let preset = number(((speed - 1) * AV1_FASTEST_PRESET + 4) / 9);
    let options = [
        (c"usage", c"allintra"),
        (c"still-picture", c"1"),
        (c"row-mt", c"1"),
        (c"crf", crf.as_c_str()),
        (c"cpu-used", preset.as_c_str()),
    ];
    let open = |pixel_format: AVPixelFormat| {
        let opening = Opening {
            name: "AV1",
            codec,
            size: (pixels.width, pixels.height),
            pixel_format,
            options: &options,
        };
        Encoder::open(format, opening, tag)
    };
    let gray = pixels.layout.is_gray();
    let color_format = match (gray, deep) {
        (true, false) => avutil::AV_PIX_FMT_GRAY8,
        (true, true) => avutil::AV_PIX_FMT_GRAY10LE,
        (false, false) => avutil::AV_PIX_FMT_YUV444P,
        (false, true) => avutil::AV_PIX_FMT_YUV444P10LE,
    };
    let mut color = open(color_format)?;
    match gray {
        true => fill_channel(&mut color, pixels, 0, deep),
        false => fill_color(&mut color, pixels, color_format)?,
    }
    let depth = if deep { 10 } else { 8 };
    let color = Plane {
        still: still(&mut color)?,
        channels: if gray { 1 } else { COLOR_CHANNELS },
        depth,
    };
    let alpha = match pixels.layout.has_alpha() {
        true => {
            let alpha_format = match deep {
                true => avutil::AV_PIX_FMT_GRAY10LE,
                false => avutil::AV_PIX_FMT_GRAY8,
            };
            let mut alpha = open(alpha_format)?;
            fill_channel(&mut alpha, pixels, pixels.layout.channels() - 1, deep);
            Some(Plane {
                still: still(&mut alpha)?,
                channels: 1,
                depth,
            })
        }
        false => None,
    };
    Picture {
        width: pixels.width,
        height: pixels.height,
        color,
        alpha,
        profile,
    }
    .to_file()
}

fn still(encoder: &mut Encoder) -> Result<Still> {
    let coded = whole_file(encoder)?;
    let extradata = unsafe {
        let context = &*encoder.context();
        match context.extradata.is_null() || context.extradata_size <= 0 {
            true => &[][..],
            false => std::slice::from_raw_parts(context.extradata, context.extradata_size as usize),
        }
    };
    Still::from_encoder(&coded, extradata)
}

fn number(value: u32) -> CString {
    CString::new(value.to_string()).unwrap_or_else(|_| unreachable!("digits hold no NUL"))
}

fn tag(ctx: &mut AVCodecContext) {
    ctx.bit_rate = 0;
    ctx.thread_count = 0;
    ctx.flags |= avcodec::AV_CODEC_FLAG_GLOBAL_HEADER;
    ctx.color_primaries = avutil::AVCOL_PRI_BT709;
    ctx.color_trc = avutil::AVCOL_TRC_IEC61966_2_1;
    ctx.colorspace = avutil::AVCOL_SPC_SMPTE170M;
    ctx.color_range = avutil::AVCOL_RANGE_JPEG;
}

fn fill_color(encoder: &mut Encoder, pixels: Pixels<'_>, to: AVPixelFormat) -> Result<()> {
    let failed = |error| Error::encoding(Format::Avif, error);
    let (width, height) = (pixels.width as c_int, pixels.height as c_int);
    let mut sws = Sws::exact();
    sws.fit(
        Raster {
            width,
            height,
            format: pixel_format(pixels.layout),
        },
        Raster {
            width,
            height,
            format: to,
        },
    )
    .map_err(failed)?;
    let matrix = Encoding {
        matrix: avutil::AVCOL_SPC_SMPTE170M,
        full_range: true,
    };
    sws.describe(matrix, matrix).map_err(failed)?;
    let planes = [pixels.bytes.as_ptr(), ptr::null(), ptr::null(), ptr::null()];
    let lines = [pixels.stride() as c_int, 0, 0, 0];
    let frame = encoder.frame();
    unsafe {
        sws.scale(
            (planes.as_ptr(), lines.as_ptr(), height),
            (frame.data().as_ptr(), frame.linesize().as_ptr(), height),
        )
    }
    .map_err(failed)
}

fn fill_channel(encoder: &mut Encoder, pixels: Pixels<'_>, channel: usize, deep: bool) {
    let channels = pixels.layout.channels();
    let width = pixels.width as usize;
    let row_bytes = width * if deep { 2 } else { 1 };
    let rows = plane_rows(encoder.frame(), 0, pixels.height, row_bytes);
    match deep {
        false => {
            for (row, source) in rows
                .into_iter()
                .zip(pixels.bytes.chunks_exact(width * channels))
            {
                for (out, pixel) in row.iter_mut().zip(source.chunks_exact(channels)) {
                    *out = pixel[channel];
                }
            }
        }
        true => {
            let pixel_bytes = channels * 2;
            let source_rows = pixels.bytes.chunks_exact(width * pixel_bytes);
            for (row, source) in rows.into_iter().zip(source_rows) {
                for (out, pixel) in row
                    .as_chunks_mut::<2>()
                    .0
                    .iter_mut()
                    .zip(source.chunks_exact(pixel_bytes))
                {
                    let at = channel * 2;
                    let sample = u16::from_ne_bytes([pixel[at], pixel[at + 1]]);
                    let ten = (u32::from(sample) * TEN_BIT_MAX + 32767) / 65535;
                    out.copy_from_slice(&(ten as u16).to_le_bytes());
                }
            }
        }
    }
}
