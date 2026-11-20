use std::ops::RangeInclusive;

use rikka_types::Choice;

use crate::key::Key;
use crate::schema::Schema;
use crate::value::Value;

pub const DOMAIN: &str = "rikka.replay";

pub const ENABLED: &str = "enabled";
pub const SUBJECT: &str = "subject";
pub const APPS: &str = "apps";
pub const LENGTH: &str = "length";
pub const MEMORY: &str = "memory";
pub const FPS: &str = "fps";
pub const QUALITY: &str = "quality";
pub const CODEC: &str = "codec";
pub const SOUND: &str = "sound";
pub const MICROPHONE: &str = "microphone";
pub const MICROPHONE_DEVICE: &str = "microphone-device";
pub const CURSOR: &str = "cursor";
pub const FOLDER: &str = "folder";

pub const ENABLED_DEFAULT: bool = false;
pub const LENGTH_DEFAULT: i64 = 120;
pub const MEMORY_DEFAULT: i64 = 1024;
pub const FPS_DEFAULT: i64 = 60;
pub const QUALITY_DEFAULT: i64 = 70;
pub const SOUND_DEFAULT: bool = true;
pub const MICROPHONE_DEFAULT: bool = false;
pub const CURSOR_DEFAULT: bool = true;

pub const UNSET: &str = "";

pub const LENGTHS: RangeInclusive<i64> = 30..=1200;
pub const LENGTH_STEP: i64 = 30;
pub const MEMORIES: RangeInclusive<i64> = 128..=16384;
pub const MEMORY_STEP: i64 = 128;
pub const FPSES: RangeInclusive<i64> = 10..=240;
pub const QUALITIES: RangeInclusive<i64> = 1..=100;

const SCREEN: &str = "screen";
const CHOSEN_APPS: &str = "apps";
const WINDOWS: &str = "windows";

const AUTO: &str = "auto";
const HEVC: &str = "hevc";
const H264: &str = "h264";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Subject {
    #[default]
    Screen,
    Apps,
    Windows,
}

impl Choice for Subject {
    const ALL: &'static [Subject] = &[Subject::Screen, Subject::Apps, Subject::Windows];

    fn slug(self) -> &'static str {
        match self {
            Subject::Screen => SCREEN,
            Subject::Apps => CHOSEN_APPS,
            Subject::Windows => WINDOWS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Codec {
    #[default]
    Auto,
    Hevc,
    H264,
}

impl Choice for Codec {
    const ALL: &'static [Codec] = &[Codec::Auto, Codec::Hevc, Codec::H264];

    fn slug(self) -> &'static str {
        match self {
            Codec::Auto => AUTO,
            Codec::Hevc => HEVC,
            Codec::H264 => H264,
        }
    }
}

#[must_use]
pub fn apps(value: &Value) -> Vec<String> {
    value.get().unwrap_or_default()
}

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary(
            "The instant replay: the last minutes of the screen, kept in memory until they are saved",
        )
        .key(Key::bool(ENABLED, ENABLED_DEFAULT).summary(
            "Whether the desktop keeps the last minutes in memory; nothing reaches the disk until a replay is saved",
        ))
        .key(Key::choice::<Subject>(SUBJECT).summary(
            "What is kept: every display, the windows of the chosen applications, or every window \
             each on its own, also behind others and on other workspaces",
        ))
        .key(Key::list(APPS, []).summary(
            "The application ids whose windows are kept when only chosen applications are",
        ))
        .key(
            Key::int(LENGTH, LENGTH_DEFAULT)
                .summary("Seconds kept back")
                .range(LENGTHS),
        )
        .key(
            Key::int(MEMORY, MEMORY_DEFAULT)
                .summary(
                    "Mebibytes of memory the replay may hold; past it the oldest seconds go first",
                )
                .range(MEMORIES),
        )
        .key(
            Key::int(FPS, FPS_DEFAULT)
                .summary("Frames a second at most; a still screen costs nothing")
                .range(FPSES),
        )
        .key(
            Key::int(QUALITY, QUALITY_DEFAULT)
                .summary("How good the replay looks — a constant quality")
                .range(QUALITIES),
        )
        .key(Key::choice::<Codec>(CODEC).summary(
            "What the replay is encoded with; automatic takes H.265 where the GPU encodes it, else H.264",
        ))
        .key(Key::bool(SOUND, SOUND_DEFAULT).summary("Whether the desktop's sound is kept"))
        .key(Key::bool(MICROPHONE, MICROPHONE_DEFAULT).summary(
            "Whether the microphone is kept too, as a track of its own",
        ))
        .key(
            Key::string(MICROPHONE_DEVICE, UNSET)
                .summary("The microphone to keep; empty for the usual one"),
        )
        .key(Key::bool(CURSOR, CURSOR_DEFAULT).summary("Whether the pointer is in the replay"))
        .key(Key::string(FOLDER, UNSET).summary(
            "Where saved replays go; empty for a Replays folder in this session's own videos",
        ))
}
