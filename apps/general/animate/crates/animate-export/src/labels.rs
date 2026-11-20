use rikka_i18n::{Label, tr};
use rikka_video::{EncodeSpeed, EncoderBackend, EncoderChoice, VideoCodec};

use crate::encoding::{Container, Rate};

pub fn container_label(container: Container) -> Label {
    Label::from(match container {
        Container::Mp4 => "MP4",
        Container::Mkv => "Matroska (MKV)",
        Container::Mov => "QuickTime (MOV)",
    })
}

pub fn codec_label(codec: VideoCodec) -> Label {
    Label::from(match codec {
        VideoCodec::H264 => "H.264",
        VideoCodec::Hevc => "H.265",
        VideoCodec::Av1 => "AV1",
        VideoCodec::Vp9 => "VP9",
    })
}

pub fn encoder_label(encoder: EncoderChoice) -> Label {
    match encoder {
        EncoderChoice::Auto => tr!("animate.export.encoder_auto"),
        EncoderChoice::Only(backend) => backend_label(backend),
    }
}

pub fn backend_label(backend: EncoderBackend) -> Label {
    match backend {
        EncoderBackend::Nvenc => Label::from("NVENC"),
        EncoderBackend::Amf => Label::from("AMF"),
        EncoderBackend::Qsv => Label::from("Quick Sync"),
        EncoderBackend::Vaapi => Label::from("VA-API"),
        EncoderBackend::VulkanVideo => Label::from("Vulkan Video"),
        EncoderBackend::Software => tr!("animate.export.encoder_software"),
    }
}

pub fn speed_label(speed: EncodeSpeed) -> Label {
    match speed {
        EncodeSpeed::Fastest => tr!("animate.export.speed_fastest"),
        EncodeSpeed::Fast => tr!("animate.export.speed_fast"),
        EncodeSpeed::Balanced => tr!("animate.export.speed_balanced"),
        EncodeSpeed::Slow => tr!("animate.export.speed_slow"),
    }
}

pub fn rate_label(rate: Rate) -> Label {
    match rate {
        Rate::Quality => tr!("animate.export.rate_quality"),
        Rate::Bitrate => tr!("animate.export.rate_bitrate"),
    }
}
