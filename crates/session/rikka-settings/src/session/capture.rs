use std::ops::RangeInclusive;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.capture";

pub const PICTURE_FORMAT: &str = "picture-format";
pub const PICTURE_QUALITY: &str = "picture-quality";
pub const PICTURE_SAVE: &str = "picture-save";
pub const PICTURE_COPY: &str = "picture-copy";
pub const PICTURE_FOLDER: &str = "picture-folder";
pub const PICTURE_CURSOR: &str = "picture-cursor";

pub const VIDEO_CONTAINER: &str = "video-container";
pub const VIDEO_CODEC: &str = "video-codec";
pub const VIDEO_ENCODER: &str = "video-encoder";
pub const VIDEO_QUALITY: &str = "video-quality";
pub const VIDEO_EFFORT: &str = "video-effort";
pub const VIDEO_FPS: &str = "video-fps";
pub const VIDEO_AUDIO: &str = "video-audio";
pub const VIDEO_AUDIO_DEVICE: &str = "video-audio-device";
pub const VIDEO_CURSOR: &str = "video-cursor";
pub const VIDEO_FOLDER: &str = "video-folder";

pub const PICTURE_FORMAT_DEFAULT: &str = "png";
pub const PICTURE_QUALITY_DEFAULT: i64 = 92;
pub const PICTURE_SAVE_DEFAULT: bool = true;
pub const PICTURE_COPY_DEFAULT: bool = true;
pub const PICTURE_CURSOR_DEFAULT: bool = false;

pub const VIDEO_CONTAINER_DEFAULT: &str = "mp4";
pub const VIDEO_CODEC_DEFAULT: &str = "h264";
pub const VIDEO_ENCODER_DEFAULT: &str = "auto";
pub const VIDEO_QUALITY_DEFAULT: i64 = 70;
pub const VIDEO_EFFORT_DEFAULT: &str = "balanced";
pub const VIDEO_FPS_DEFAULT: i64 = 60;
pub const VIDEO_AUDIO_DEFAULT: &str = "none";
pub const VIDEO_CURSOR_DEFAULT: bool = true;

pub const UNSET: &str = "";

pub const QUALITY: RangeInclusive<i64> = 1..=100;
pub const FPS: RangeInclusive<i64> = 1..=240;

pub const PICTURE_FORMATS: [&str; 4] = ["png", "jpeg", "webp", "avif"];
pub const CONTAINERS: [&str; 3] = ["mp4", "mkv", "webm"];
pub const CODECS: [&str; 3] = ["h264", "hevc", "av1"];
pub const ENCODERS: [&str; 7] = ["auto", "nvenc", "amf", "qsv", "vaapi", "vulkan", "software"];
pub const EFFORTS: [&str; 4] = ["fastest", "fast", "balanced", "slow"];
pub const AUDIO: [&str; 3] = ["none", "microphone", "system"];

pub const WEBM: &str = "webm";
pub const AV1: &str = "av1";

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("What the desktop does with a picture of the screen, and with a recording of it")
        .key(
            Key::string(PICTURE_FORMAT, PICTURE_FORMAT_DEFAULT)
                .summary("What a screenshot is written as")
                .choices(PICTURE_FORMATS),
        )
        .key(
            Key::int(PICTURE_QUALITY, PICTURE_QUALITY_DEFAULT)
                .summary("How good a screenshot looks where the format is a lossy one")
                .range(QUALITY),
        )
        .key(
            Key::bool(PICTURE_SAVE, PICTURE_SAVE_DEFAULT)
                .summary("Whether a screenshot is written to a file"),
        )
        .key(
            Key::bool(PICTURE_COPY, PICTURE_COPY_DEFAULT)
                .summary("Whether a screenshot is put on the clipboard"),
        )
        .key(Key::string(PICTURE_FOLDER, UNSET).summary(
            "Where screenshots are written; empty for a Screenshots folder in this session's own pictures",
        ))
        .key(
            Key::bool(PICTURE_CURSOR, PICTURE_CURSOR_DEFAULT)
                .summary("Whether the pointer is in the picture"),
        )
        .key(
            Key::string(VIDEO_CONTAINER, VIDEO_CONTAINER_DEFAULT)
                .summary("What a recording is written as; WebM holds AV1 and nothing else")
                .choices(CONTAINERS),
        )
        .key(
            Key::string(VIDEO_CODEC, VIDEO_CODEC_DEFAULT)
                .summary("What a recording is encoded with; H.264 is the one everything plays")
                .choices(CODECS),
        )
        .key(
            Key::string(VIDEO_ENCODER, VIDEO_ENCODER_DEFAULT)
                .summary("Which encoder does it; automatic is the GPU's own where there is one")
                .choices(ENCODERS),
        )
        .key(
            Key::int(VIDEO_QUALITY, VIDEO_QUALITY_DEFAULT)
                .summary("How good a recording looks — a constant quality, so a still screen costs almost nothing")
                .range(QUALITY),
        )
        .key(
            Key::string(VIDEO_EFFORT, VIDEO_EFFORT_DEFAULT)
                .summary("How much of the machine the encoder may spend making the file smaller")
                .choices(EFFORTS),
        )
        .key(
            Key::int(VIDEO_FPS, VIDEO_FPS_DEFAULT)
                .summary("Frames a second")
                .range(FPS),
        )
        .key(
            Key::string(VIDEO_AUDIO, VIDEO_AUDIO_DEFAULT)
                .summary("What a recording records the sound of")
                .choices(AUDIO),
        )
        .key(
            Key::string(VIDEO_AUDIO_DEVICE, UNSET)
                .summary("The capture device to record the sound of; empty for the usual one"),
        )
        .key(
            Key::bool(VIDEO_CURSOR, VIDEO_CURSOR_DEFAULT)
                .summary("Whether the pointer is in the recording"),
        )
        .key(Key::string(VIDEO_FOLDER, UNSET).summary(
            "Where recordings are written; empty for a Screencasts folder in this session's own videos",
        ))
}
