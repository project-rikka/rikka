use std::path::PathBuf;

use rikka_audio::CaptureSource;
use rikka_image::Format;
pub(crate) use rikka_settings::session::capture::*;
use rikka_video::{EncodeSpeed, EncoderBackend, EncoderChoice, VideoCodec};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Container {
    #[default]
    Mp4,
    Matroska,
    WebM,
}

impl Container {
    #[must_use]
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Matroska => "mkv",
            Container::WebM => "webm",
        }
    }

    #[must_use]
    pub(crate) fn holding(self, codec: VideoCodec) -> VideoCodec {
        if self != Container::WebM || codec == VideoCodec::Av1 {
            return codec;
        }
        tracing::info!("WebM does not support {codec:?}, recording AV1 instead");
        VideoCodec::Av1
    }
}

#[must_use]
pub(super) fn picture_format(named: &str) -> Format {
    match named {
        "jpeg" => Format::Jpeg,
        "webp" => Format::WebP,
        "avif" => Format::Avif,
        _ => Format::Png,
    }
}

#[must_use]
pub(crate) fn container(named: &str) -> Container {
    match named {
        "mkv" => Container::Matroska,
        "webm" => Container::WebM,
        _ => Container::Mp4,
    }
}

#[must_use]
pub(super) fn codec(named: &str) -> VideoCodec {
    match named {
        "hevc" => VideoCodec::Hevc,
        "av1" => VideoCodec::Av1,
        _ => VideoCodec::H264,
    }
}

#[must_use]
pub(super) fn encoder(named: &str) -> EncoderChoice {
    let backend = match named {
        "nvenc" => EncoderBackend::Nvenc,
        "amf" => EncoderBackend::Amf,
        "qsv" => EncoderBackend::Qsv,
        "vaapi" => EncoderBackend::Vaapi,
        "vulkan" => EncoderBackend::VulkanVideo,
        "software" => EncoderBackend::Software,
        _ => return EncoderChoice::Auto,
    };
    EncoderChoice::Only(backend)
}

#[must_use]
pub(crate) fn effort(named: &str) -> EncodeSpeed {
    match named {
        "fastest" => EncodeSpeed::Fastest,
        "fast" => EncodeSpeed::Fast,
        "slow" => EncodeSpeed::Slow,
        _ => EncodeSpeed::Balanced,
    }
}

#[must_use]
pub(crate) fn audio(named: &str, device: &str) -> Option<CaptureSource> {
    let kind = match named {
        "microphone" => CaptureSource::Microphone,
        "system" => CaptureSource::SystemAudio,
        _ => return None,
    };
    match device.trim().is_empty() {
        true => Some(kind),
        false => Some(CaptureSource::Named(device.to_owned())),
    }
}

#[must_use]
pub(crate) fn folder(named: &str) -> Option<PathBuf> {
    let named = named.trim();
    (!named.is_empty() && named.starts_with('/')).then(|| PathBuf::from(named))
}

#[must_use]
pub(crate) fn quality(value: i64) -> u8 {
    value.clamp(*QUALITY.start(), *QUALITY.end()) as u8
}

#[must_use]
pub(super) fn fps(value: i64) -> u32 {
    value.clamp(*FPS.start(), *FPS.end()) as u32
}
