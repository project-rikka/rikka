use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avcodec::{
    AV_SAMPLE_FMT_DBL, AV_SAMPLE_FMT_DBLP, AV_SAMPLE_FMT_FLT, AV_SAMPLE_FMT_FLTP,
    AV_SAMPLE_FMT_S16, AV_SAMPLE_FMT_S16P, AV_SAMPLE_FMT_S32, AV_SAMPLE_FMT_S32P,
    AV_SAMPLE_FMT_S64, AV_SAMPLE_FMT_S64P, AV_SAMPLE_FMT_U8, AV_SAMPLE_FMT_U8P,
};

use crate::error::{Error, Result};

#[derive(Clone, Copy)]
enum Kind {
    U8,
    S16,
    S32,
    S64,
    F32,
    F64,
}

impl Kind {
    fn width(self) -> usize {
        match self {
            Kind::U8 => 1,
            Kind::S16 => 2,
            Kind::S32 | Kind::F32 => 4,
            Kind::S64 | Kind::F64 => 8,
        }
    }

    fn read(self, bytes: &[u8]) -> f32 {
        match self {
            Kind::U8 => (f32::from(bytes[0]) - 128.0) / 128.0,
            Kind::S16 => f32::from(i16::from_ne_bytes([bytes[0], bytes[1]])) / 32_768.0,
            Kind::S32 => {
                (f64::from(i32::from_ne_bytes(
                    bytes[..4].try_into().unwrap_or_default(),
                )) / 2_147_483_648.0) as f32
            }
            Kind::S64 => {
                (i64::from_ne_bytes(bytes[..8].try_into().unwrap_or_default()) as f64
                    / 9_223_372_036_854_775_808.0) as f32
            }
            Kind::F32 => f32::from_ne_bytes(bytes[..4].try_into().unwrap_or_default()),
            Kind::F64 => f64::from_ne_bytes(bytes[..8].try_into().unwrap_or_default()) as f32,
        }
    }
}

pub(crate) fn interleave(frame: &Frame, out: &mut Vec<f32>) -> Result<()> {
    let (kind, planar) = match frame.format() {
        AV_SAMPLE_FMT_U8 => (Kind::U8, false),
        AV_SAMPLE_FMT_S16 => (Kind::S16, false),
        AV_SAMPLE_FMT_S32 => (Kind::S32, false),
        AV_SAMPLE_FMT_S64 => (Kind::S64, false),
        AV_SAMPLE_FMT_FLT => (Kind::F32, false),
        AV_SAMPLE_FMT_DBL => (Kind::F64, false),
        AV_SAMPLE_FMT_U8P => (Kind::U8, true),
        AV_SAMPLE_FMT_S16P => (Kind::S16, true),
        AV_SAMPLE_FMT_S32P => (Kind::S32, true),
        AV_SAMPLE_FMT_S64P => (Kind::S64, true),
        AV_SAMPLE_FMT_FLTP => (Kind::F32, true),
        AV_SAMPLE_FMT_DBLP => (Kind::F64, true),
        other => return Err(Error::SampleFormat(other)),
    };
    let channels = frame.channels();
    let frames = frame.samples();
    let width = kind.width();
    let data = frame.extended_data();
    if frames == 0 || channels == 0 || data.is_null() {
        return Ok(());
    }
    out.reserve(frames * channels);
    if planar {
        let planes: Vec<&[u8]> = (0..channels)
            .map(|channel| unsafe {
                std::slice::from_raw_parts(*data.add(channel), frames * width)
            })
            .collect();
        for index in 0..frames {
            for plane in &planes {
                out.push(kind.read(&plane[index * width..]));
            }
        }
    } else {
        let packed = unsafe { std::slice::from_raw_parts(*data, frames * channels * width) };
        out.extend(packed.chunks_exact(width).map(|sample| kind.read(sample)));
    }
    Ok(())
}
