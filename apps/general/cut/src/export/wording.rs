use cut_export::{
    Compression, Container, FrameRate, Phase, Preset, Range, Resolution, SoundCodec, StillFormat,
    Target,
};
use cut_project::{Format, Rate};
use rikka_app::prelude::*;
use rikka_audio_decode::WavDepth;
use rikka_video::{EncodeSpeed, EncoderBackend, EncoderChoice, VideoCodec};

pub(crate) fn target(target: Target) -> Label {
    match target {
        Target::Video => tr!("cut.export.target_video"),
        Target::Audio => tr!("cut.export.target_audio"),
        Target::Still => tr!("cut.export.target_still"),
    }
}

pub(crate) fn preset(preset: Preset) -> Label {
    match preset {
        Preset::High => tr!("cut.export.preset_high"),
        Preset::Standard => tr!("cut.export.preset_standard"),
        Preset::Small => tr!("cut.export.preset_small"),
    }
}

pub(crate) fn range(range: Range) -> Label {
    match range {
        Range::Whole => tr!("cut.export.range_whole"),
        Range::Marks => tr!("cut.export.range_marks"),
    }
}

pub(crate) fn container(container: Container) -> Label {
    Label::from(match container {
        Container::Mp4 => "MP4",
        Container::Mov => "QuickTime (MOV)",
        Container::Mkv => "Matroska (MKV)",
    })
}

pub(crate) fn codec(codec: VideoCodec) -> Label {
    Label::from(match codec {
        VideoCodec::H264 => "H.264",
        VideoCodec::Hevc => "H.265 (HEVC)",
        VideoCodec::Av1 => "AV1",
        VideoCodec::Vp9 => "VP9",
    })
}

pub(crate) fn encoder(encoder: EncoderChoice) -> Label {
    match encoder {
        EncoderChoice::Auto => tr!("cut.export.encoder_auto"),
        EncoderChoice::Only(backend) => self::backend(backend),
    }
}

pub(crate) fn backend(backend: EncoderBackend) -> Label {
    match backend {
        EncoderBackend::Nvenc => Label::from("NVENC"),
        EncoderBackend::Amf => Label::from("AMF"),
        EncoderBackend::Qsv => Label::from("Quick Sync"),
        EncoderBackend::Vaapi => Label::from("VA-API"),
        EncoderBackend::VulkanVideo => Label::from("Vulkan Video"),
        EncoderBackend::Software => tr!("cut.export.encoder_software"),
    }
}

pub(crate) fn speed(speed: EncodeSpeed) -> Label {
    match speed {
        EncodeSpeed::Fastest => tr!("cut.export.speed_fastest"),
        EncodeSpeed::Fast => tr!("cut.export.speed_fast"),
        EncodeSpeed::Balanced => tr!("cut.export.speed_balanced"),
        EncodeSpeed::Slow => tr!("cut.export.speed_slow"),
    }
}

pub(crate) fn compression(compression: Compression) -> Label {
    match compression {
        Compression::Quality => tr!("cut.export.compression_quality"),
        Compression::Bitrate => tr!("cut.export.compression_bitrate"),
    }
}

pub(crate) fn size((width, height): (u32, u32)) -> String {
    format!("{width}×{height}")
}

pub(crate) fn resolution(resolution: Resolution, format: Format) -> Label {
    let dimensions = size(resolution.size(format));
    match resolution {
        Resolution::Project => tr!("cut.export.same_as_project", value = dimensions.as_str()),
        Resolution::Short(_) => Label::from(dimensions),
    }
}

pub(crate) fn rate(rate: Rate) -> String {
    format!("{rate} fps")
}

pub(crate) fn frame_rate(frame_rate: FrameRate, project: Rate) -> Label {
    match frame_rate {
        FrameRate::Project => tr!("cut.export.same_as_project", value = rate(project).as_str()),
        FrameRate::Fixed(fixed) => Label::from(rate(fixed)),
    }
}

pub(crate) fn sound_codec(codec: SoundCodec) -> Label {
    Label::from(match codec {
        SoundCodec::Wav => "WAV",
        SoundCodec::Aac => "AAC (M4A)",
    })
}

pub(crate) fn depth(depth: WavDepth) -> Label {
    Label::from(match depth {
        WavDepth::Pcm16 => "16-bit",
        WavDepth::Pcm24 => "24-bit",
    })
}

pub(crate) fn sample_rate(rate: u32) -> String {
    let kilo = f64::from(rate) / 1000.0;
    let formatted = format!("{kilo:.1}");
    format!("{} kHz", formatted.trim_end_matches(".0"))
}

pub(crate) fn kilobits(bitrate: u32) -> String {
    format!("{bitrate} kbit/s")
}

pub(crate) fn still_format(format: StillFormat) -> Label {
    Label::from(match format {
        StillFormat::Png => "PNG",
        StillFormat::Jpeg => "JPEG",
    })
}

pub(crate) fn phase(phase: &Phase) -> Label {
    match phase {
        Phase::Preparing => tr!("cut.export.preparing"),
        Phase::Rendering => tr!("cut.export.rendering"),
        Phase::Finishing => tr!("cut.export.finishing"),
        Phase::Cancelling | Phase::Cancelled => tr!("cut.export.cancelling"),
        Phase::Done(_) => tr!("cut.export.done"),
        Phase::Failed(_) => tr!("cut.export.failed"),
    }
}

pub(crate) fn joined(facts: &[Label]) -> String {
    facts
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(" · ")
}
