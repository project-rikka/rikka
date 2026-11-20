use std::sync::OnceLock;

use crate::error::{Error, Result};
use crate::{avcodec, avformat, avutil, swscale};

static CODECS: OnceLock<std::result::Result<(), String>> = OnceLock::new();
static SCALING: OnceLock<std::result::Result<(), String>> = OnceLock::new();

pub fn load() -> Result<()> {
    let load_result = CODECS.get_or_init(|| {
        avutil::load()?;
        avcodec::load()?;
        avformat::load()?;
        unsafe { avutil::av_log_set_level(log_level()) };
        Ok(())
    });
    load_result.clone().map_err(Error::Unavailable)
}

pub fn load_scaling() -> Result<()> {
    load()?;
    SCALING
        .get_or_init(swscale::load)
        .clone()
        .map_err(Error::Unavailable)
}

pub fn version() -> Result<String> {
    load()?;
    let (utility, load_result, formats) = unsafe {
        (
            avutil::avutil_version(),
            avcodec::avcodec_version(),
            avformat::avformat_version(),
        )
    };
    let dotted = |packed: u32| {
        format!(
            "{}.{}.{}",
            packed >> 16,
            (packed >> 8) & 0xff,
            packed & 0xff
        )
    };
    Ok(format!(
        "libavutil {} / libavcodec {} / libavformat {}",
        dotted(utility),
        dotted(load_result),
        dotted(formats)
    ))
}

fn log_level() -> core::ffi::c_int {
    match std::env::var("RIKKA_FFMPEG_LOG").as_deref() {
        Ok("quiet") => avutil::AV_LOG_QUIET,
        Ok("warning") => avutil::AV_LOG_WARNING,
        Ok("info") => avutil::AV_LOG_INFO,
        Ok("verbose") => avutil::AV_LOG_VERBOSE,
        Ok("debug") => avutil::AV_LOG_DEBUG,
        _ => avutil::AV_LOG_ERROR,
    }
}
