use std::path::PathBuf;
use std::time::Duration;

use rikka_settings::seam::Settings;
use rikka_settings::session::replay::{self, Codec, Subject};
use rikka_video::VideoCodec;

const MEBIBYTE: usize = 1 << 20;
const MAX_MEMORY_DIVISOR: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CaptureOptions {
    pub(crate) subject: Subject,
    pub(crate) apps: Vec<String>,
    pub(crate) fps: u32,
    pub(crate) quality: u8,
    pub(crate) codec: Codec,
    pub(crate) cursor: bool,
}

impl CaptureOptions {
    #[must_use]
    pub(crate) fn codecs(&self) -> &'static [VideoCodec] {
        match self.codec {
            Codec::Auto => &[VideoCodec::Hevc, VideoCodec::H264],
            Codec::Hevc => &[VideoCodec::Hevc],
            Codec::H264 => &[VideoCodec::H264],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AudioOptions {
    pub(crate) desktop: bool,
    pub(crate) microphone: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) enabled: bool,
    pub(crate) capture: CaptureOptions,
    pub(crate) audio: AudioOptions,
    pub(crate) length: Duration,
    pub(crate) budget: usize,
    pub(crate) folder: Option<PathBuf>,
}

impl Options {
    #[must_use]
    pub(crate) fn read(settings: &Settings) -> Options {
        let count = |key: &str, declared: i64| settings.read_or::<i64>(key, declared);
        let flag = |key: &str, declared: bool| settings.read_or::<bool>(key, declared);
        let text = |key: &str| settings.read_or::<String>(key, String::new());
        let apps = settings
            .value(replay::APPS)
            .map(replay::apps)
            .unwrap_or_default();
        let device = text(replay::MICROPHONE_DEVICE);
        let folder = text(replay::FOLDER);
        Options {
            enabled: flag(replay::ENABLED, replay::ENABLED_DEFAULT),
            capture: CaptureOptions {
                subject: settings.choice(replay::SUBJECT),
                apps,
                fps: clamp_to(count(replay::FPS, replay::FPS_DEFAULT), &replay::FPSES) as u32,
                quality: clamp_to(
                    count(replay::QUALITY, replay::QUALITY_DEFAULT),
                    &replay::QUALITIES,
                ) as u8,
                codec: settings.choice(replay::CODEC),
                cursor: flag(replay::CURSOR, replay::CURSOR_DEFAULT),
            },
            audio: AudioOptions {
                desktop: flag(replay::SOUND, replay::SOUND_DEFAULT),
                microphone: flag(replay::MICROPHONE, replay::MICROPHONE_DEFAULT)
                    .then(|| device.trim().to_owned()),
            },
            length: Duration::from_secs(clamp_to(
                count(replay::LENGTH, replay::LENGTH_DEFAULT),
                &replay::LENGTHS,
            ) as u64),
            budget: memory_budget(clamp_to(
                count(replay::MEMORY, replay::MEMORY_DEFAULT),
                &replay::MEMORIES,
            ) as usize),
            folder: Some(PathBuf::from(folder.trim())).filter(|path| path.is_absolute()),
        }
    }
}

fn clamp_to(value: i64, range: &std::ops::RangeInclusive<i64>) -> i64 {
    value.clamp(*range.start(), *range.end())
}

fn memory_budget(mebibytes: usize) -> usize {
    let requested = mebibytes.saturating_mul(MEBIBYTE);
    let info = rustix::system::sysinfo();
    let memory = (info.totalram as usize).saturating_mul(info.mem_unit as usize);
    requested.min(memory / MAX_MEMORY_DIVISOR)
}
