use std::time::Duration;

use rikka_ffmpeg::avcodec::{self, AVCodecContext};

use crate::encode::backend::Encoder;
use crate::encode::config::{Latency, RateControl, Spec, VideoCodec};
use crate::encode::quality::{self, Family, set};

const ENDLESS: i32 = i32::MAX;
const X265_ENDLESS: &str = "keyint=-1:scenecut=0";
const X264: &str = "libx264";
const CAPPED_VBV: Duration = Duration::from_millis(500);

pub(crate) fn apply(raw: *mut AVCodecContext, encoder: Encoder, spec: &Spec) {
    unsafe { (*raw).gop_size = gop(encoder, spec) };
    match spec.latency {
        Latency::Buffered => {
            if let RateControl::Bitrate(bitrate) = spec.rate {
                unsafe { (*raw).bit_rate = bitrate as i64 };
            }
        }
        Latency::Immediate => {
            unsafe {
                (*raw).max_b_frames = 0;
                (*raw).flags |= avcodec::AV_CODEC_FLAG_LOW_DELAY;
            }
            rate(raw, spec);
        }
    }
    match (encoder.family, spec.rate) {
        (Some(family), RateControl::Quality(level)) => {
            quality::apply(raw, family, level);
            quality::pace(raw, family, spec.speed);
        }
        (Some(family), RateControl::Default | RateControl::Bitrate(_)) => {
            quality::pace(raw, family, spec.speed);
        }
        (None, RateControl::Quality(_)) => tracing::debug!(
            "{} has no constant-quality mode, keeping default rate control",
            encoder.name
        ),
        (None, RateControl::Default | RateControl::Bitrate(_)) => {}
    }
    if spec.latency == Latency::Immediate {
        immediate(raw, encoder, spec);
    }
}

pub(crate) fn rate(raw: *mut AVCodecContext, spec: &Spec) {
    let RateControl::Bitrate(bitrate) = spec.rate else {
        return;
    };
    let buffer = (bitrate as f64 * CAPPED_VBV.as_secs_f64()).round() as i32;
    unsafe {
        (*raw).bit_rate = bitrate as i64;
        (*raw).rc_max_rate = bitrate as i64;
        (*raw).rc_buffer_size = buffer.max(1);
    }
}

pub(crate) fn retunes_in_place(encoder: Encoder) -> bool {
    match encoder.family {
        Some(Family::Nvenc | Family::Qsv) => true,
        Some(Family::X26x) => encoder.name == X264,
        _ => false,
    }
}

fn gop(encoder: Encoder, spec: &Spec) -> i32 {
    match (spec.keyframe_period(), encoder.family, spec.codec) {
        (Some(period), _, _) => period,
        (None, Some(Family::X26x), VideoCodec::Hevc) => -1,
        (None, _, _) => ENDLESS,
    }
}

fn immediate(raw: *mut AVCodecContext, encoder: Encoder, spec: &Spec) {
    let options = unsafe { (*raw).priv_data };
    if options.is_null() {
        return;
    }
    let set = |key: &str, value: &str| set(options, key, value);
    let bitrate = matches!(spec.rate, RateControl::Bitrate(_));
    let endless = spec.keyframe_period().is_none();
    match encoder.family {
        Some(Family::X26x) => {
            set("tune", "zerolatency");
            set("forced-idr", "1");
            match (spec.codec, endless) {
                (VideoCodec::H264, true) => set("sc_threshold", "0"),
                (VideoCodec::Hevc, true) => set("x265-params", X265_ENDLESS),
                _ => {}
            }
        }
        Some(Family::Nvenc) => {
            set("tune", "ull");
            set("zerolatency", "1");
            set("delay", "0");
            set("rc-lookahead", "0");
            set("forced-idr", "1");
            if bitrate {
                set("rc", "vbr");
            }
        }
        Some(Family::Amf) => {
            set("usage", "ultralowlatency");
            if bitrate {
                set("rc", "cbr");
            }
        }
        Some(Family::Qsv) => {
            set("async_depth", "1");
            set("forced_idr", "1");
            if bitrate {
                set("low_delay_brc", "1");
            }
        }
        Some(Family::Vaapi) => {
            set("async_depth", "1");
            if bitrate {
                set("rc_mode", "CBR");
            }
        }
        Some(Family::SvtAv1 | Family::Rav1e | Family::Aom | Family::Vpx | Family::Kvazaar) => {}
        None => {
            if bitrate {
                set("rc_mode", "bitrate");
                set("allow_skip_frames", "1");
            }
        }
    }
}
