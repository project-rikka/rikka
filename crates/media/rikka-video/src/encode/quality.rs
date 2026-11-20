use std::ffi::CString;

use crate::error::{Error, Result};
use rikka_ffmpeg::avcodec::AVCodecContext;
use rikka_ffmpeg::avutil;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EncodeSpeed {
    Fastest,
    Fast,
    #[default]
    Balanced,
    Slow,
}

const QSCALE: i32 = rikka_ffmpeg::avcodec::AV_CODEC_FLAG_QSCALE;

const QP2LAMBDA: i32 = rikka_ffmpeg::avcodec::FF_QP2LAMBDA;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    X26x,
    SvtAv1,
    Rav1e,
    Aom,
    Vpx,
    Kvazaar,
    Nvenc,
    Amf,
    Qsv,
    Vaapi,
}

impl Family {
    #[must_use]
    fn needs_no_bitrate(self) -> bool {
        matches!(
            self,
            Family::Nvenc | Family::Amf | Family::Vaapi | Family::Vpx
        )
    }
}

pub(crate) fn apply(codec_ctx: *mut AVCodecContext, family: Family, quality: u8) {
    let options = unsafe { (*codec_ctx).priv_data };
    if options.is_null() {
        return;
    }

    if family.needs_no_bitrate() {
        unsafe { (*codec_ctx).bit_rate = 0 };
    }

    let cq = constant_quality(family, quality).to_string();
    match family {
        Family::X26x | Family::SvtAv1 | Family::Aom | Family::Vpx => set(options, "crf", &cq),
        Family::Rav1e | Family::Kvazaar => set(options, "qp", &cq),
        Family::Nvenc => {
            set(options, "rc", "vbr");
            set(options, "cq", &cq);
        }
        Family::Amf => {
            set(options, "rc", "cqp");
            for key in ["qp_i", "qp_p", "qp_b"] {
                set(options, key, &cq);
            }
        }
        Family::Qsv => unsafe {
            (*codec_ctx).global_quality = constant_quality(family, quality) * QP2LAMBDA;
            (*codec_ctx).flags |= QSCALE;
        },
        Family::Vaapi => {
            set(options, "rc_mode", "CQP");
            set(options, "qp", &cq);
        }
    }
}

pub(crate) fn pace(codec_ctx: *mut AVCodecContext, family: Family, speed: EncodeSpeed) {
    let options = unsafe { (*codec_ctx).priv_data };
    if options.is_null() {
        return;
    }
    match family {
        Family::X26x | Family::Qsv => set(options, "preset", named_preset(speed)),
        Family::SvtAv1 => set(options, "preset", &svt_preset(speed).to_string()),
        Family::Rav1e => set(options, "speed", &rav1e_speed(speed).to_string()),
        Family::Aom => set(options, "cpu-used", &aom_cpu_used(speed).to_string()),
        Family::Vpx => {
            let (deadline, cpu_used) = vpx_pace(speed);
            set(options, "deadline", deadline);
            set(options, "cpu-used", &cpu_used.to_string());
            set(options, "row-mt", "1");
            set(options, "tile-columns", "2");
        }
        Family::Nvenc => {
            set(options, "preset", nvenc_preset(speed));
            set(options, "tune", "hq");
        }
        Family::Amf => set(options, "quality", amf_quality(speed)),
        Family::Kvazaar | Family::Vaapi => {}
    }
}

#[must_use]
const fn vpx_pace(speed: EncodeSpeed) -> (&'static str, i32) {
    match speed {
        EncodeSpeed::Fastest => ("realtime", 8),
        EncodeSpeed::Fast => ("good", 5),
        EncodeSpeed::Balanced => ("good", 4),
        EncodeSpeed::Slow => ("good", 2),
    }
}

#[must_use]
fn constant_quality(family: Family, quality: u8) -> i32 {
    let quality = quality.clamp(1, 100) as i32;
    match family {
        Family::X26x
        | Family::Kvazaar
        | Family::Nvenc
        | Family::Amf
        | Family::Qsv
        | Family::Vaapi => (36 - quality * 20 / 100).clamp(1, 51),
        Family::SvtAv1 | Family::Aom | Family::Vpx => (52 - quality * 26 / 100).clamp(1, 63),
        Family::Rav1e => (160 - quality * 80 / 100).clamp(1, 255),
    }
}

#[cfg(unix)]
pub(crate) fn coded_qp(quality: u8) -> u8 {
    constant_quality(Family::X26x, quality) as u8
}

#[must_use]
const fn named_preset(speed: EncodeSpeed) -> &'static str {
    match speed {
        EncodeSpeed::Fastest => "ultrafast",
        EncodeSpeed::Fast => "veryfast",
        EncodeSpeed::Balanced => "fast",
        EncodeSpeed::Slow => "slow",
    }
}

#[must_use]
const fn svt_preset(speed: EncodeSpeed) -> i32 {
    match speed {
        EncodeSpeed::Fastest => 11,
        EncodeSpeed::Fast => 9,
        EncodeSpeed::Balanced => 7,
        EncodeSpeed::Slow => 5,
    }
}

#[must_use]
const fn rav1e_speed(speed: EncodeSpeed) -> i32 {
    match speed {
        EncodeSpeed::Fastest => 10,
        EncodeSpeed::Fast => 8,
        EncodeSpeed::Balanced => 6,
        EncodeSpeed::Slow => 4,
    }
}

#[must_use]
const fn aom_cpu_used(speed: EncodeSpeed) -> i32 {
    match speed {
        EncodeSpeed::Fastest => 8,
        EncodeSpeed::Fast => 7,
        EncodeSpeed::Balanced => 6,
        EncodeSpeed::Slow => 4,
    }
}

#[must_use]
const fn nvenc_preset(speed: EncodeSpeed) -> &'static str {
    match speed {
        EncodeSpeed::Fastest => "p1",
        EncodeSpeed::Fast => "p3",
        EncodeSpeed::Balanced => "p4",
        EncodeSpeed::Slow => "p6",
    }
}

#[must_use]
const fn amf_quality(speed: EncodeSpeed) -> &'static str {
    match speed {
        EncodeSpeed::Fastest | EncodeSpeed::Fast => "speed",
        EncodeSpeed::Balanced => "balanced",
        EncodeSpeed::Slow => "quality",
    }
}

pub(crate) fn set(options: *mut std::ffi::c_void, key: &str, value: &str) {
    let (Ok(key_c), Ok(value_c)) = (CString::new(key), CString::new(value)) else {
        return;
    };
    let result = unsafe { avutil::av_opt_set(options, key_c.as_ptr(), value_c.as_ptr(), 0) };
    if result < 0 {
        tracing::debug!("encoder rejected option {key}={value}");
    }
}

pub(crate) fn checked(quality: u8) -> Result<()> {
    match (1..=100).contains(&quality) {
        true => Ok(()),
        false => Err(Error::Unsupported(format!(
            "quality {quality} (must be 1..=100)"
        ))),
    }
}
