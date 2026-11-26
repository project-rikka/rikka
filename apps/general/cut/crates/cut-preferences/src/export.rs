use std::path::PathBuf;

use cut_export::{
    AAC_BITRATES, Audio, Compression, Container, FrameRate, Resolution, Settings, SoundCodec,
    Still, StillFormat, Target, Video,
};
use cut_project::Rate;
use rikka_audio_decode::WavDepth;
use rikka_settings::{Key, Role, Schema};
use rikka_video::{EncodeSpeed, EncoderBackend, EncoderChoice, VideoCodec};

use super::{Preferences, choice, name_of, named};

const TARGET: &str = "export-target";
const FOLDER: &str = "export-folder";
const CONTAINER: &str = "video-container";
const CODEC: &str = "video-codec";
const ENCODER: &str = "video-encoder";
const SPEED: &str = "video-speed";
const COMPRESSION: &str = "video-compression";
const QUALITY: &str = "video-quality";
const BITRATE: &str = "video-bitrate";
const RESOLUTION: &str = "video-resolution";
const FRAME_RATE: &str = "video-frame-rate";
const SOUND: &str = "video-sound";
const SOUND_BITRATE: &str = "video-sound-bitrate";
const AUDIO_CODEC: &str = "audio-codec";
const AUDIO_DEPTH: &str = "audio-depth";
const AUDIO_SAMPLE_RATE: &str = "audio-sample-rate";
const AUDIO_BITRATE: &str = "audio-bitrate";
const STILL_FORMAT: &str = "still-format";
const STILL_RESOLUTION: &str = "still-resolution";
const PROJECT: &str = "project";

const TARGETS: [(Target, &str); 3] = [
    (Target::Video, "video"),
    (Target::Audio, "audio"),
    (Target::Still, "still"),
];
const CONTAINERS: [(Container, &str); 3] = [
    (Container::Mp4, "mp4"),
    (Container::Mov, "mov"),
    (Container::Mkv, "mkv"),
];
const CODECS: [(VideoCodec, &str); 3] = [
    (VideoCodec::H264, "h264"),
    (VideoCodec::Hevc, "hevc"),
    (VideoCodec::Av1, "av1"),
];
const ENCODERS: [(EncoderChoice, &str); 6] = [
    (EncoderChoice::Auto, "auto"),
    (EncoderChoice::Only(EncoderBackend::Nvenc), "nvenc"),
    (EncoderChoice::Only(EncoderBackend::Amf), "amf"),
    (EncoderChoice::Only(EncoderBackend::Qsv), "qsv"),
    (EncoderChoice::Only(EncoderBackend::Vaapi), "vaapi"),
    (EncoderChoice::Only(EncoderBackend::Software), "software"),
];
const SPEEDS: [(EncodeSpeed, &str); 4] = [
    (EncodeSpeed::Fastest, "fastest"),
    (EncodeSpeed::Fast, "fast"),
    (EncodeSpeed::Balanced, "balanced"),
    (EncodeSpeed::Slow, "slow"),
];
const COMPRESSIONS: [(Compression, &str); 2] = [
    (Compression::Quality, "quality"),
    (Compression::Bitrate, "bitrate"),
];
const SOUND_CODECS: [(SoundCodec, &str); 2] = [(SoundCodec::Wav, "wav"), (SoundCodec::Aac, "aac")];
const DEPTHS: [(WavDepth, &str); 2] = [(WavDepth::Pcm16, "16"), (WavDepth::Pcm24, "24")];
const STILL_FORMATS: [(StillFormat, &str); 2] =
    [(StillFormat::Png, "png"), (StillFormat::Jpeg, "jpeg")];

impl Preferences {
    #[must_use]
    pub fn export(&self) -> Settings {
        let fallback = Settings::default();
        let store = &self.0;
        let choice = |key: &str| store.get::<String>(key);
        let number = |key: &str| store.get::<u32>(key);
        let video = Video {
            container: named(&CONTAINERS, &choice(CONTAINER)).unwrap_or(fallback.video.container),
            codec: named(&CODECS, &choice(CODEC)).unwrap_or(fallback.video.codec),
            encoder: named(&ENCODERS, &choice(ENCODER)).unwrap_or(fallback.video.encoder),
            speed: named(&SPEEDS, &choice(SPEED)).unwrap_or(fallback.video.speed),
            compression: named(&COMPRESSIONS, &choice(COMPRESSION))
                .unwrap_or(fallback.video.compression),
            quality: u8::try_from(number(QUALITY))
                .ok()
                .filter(|quality| Video::QUALITIES.contains(quality))
                .unwrap_or(fallback.video.quality),
            bitrate: Some(number(BITRATE))
                .filter(|bitrate| Video::BITRATES.contains(bitrate))
                .unwrap_or(fallback.video.bitrate),
            resolution: resolution(&choice(RESOLUTION)).unwrap_or(fallback.video.resolution),
            frame_rate: frame_rate(&choice(FRAME_RATE)).unwrap_or(fallback.video.frame_rate),
            sound: store.get(SOUND),
            sound_bitrate: Some(number(SOUND_BITRATE))
                .filter(|bitrate| AAC_BITRATES.contains(bitrate))
                .unwrap_or(fallback.video.sound_bitrate),
        };
        let audio = Audio {
            codec: named(&SOUND_CODECS, &choice(AUDIO_CODEC)).unwrap_or(fallback.audio.codec),
            depth: named(&DEPTHS, &choice(AUDIO_DEPTH)).unwrap_or(fallback.audio.depth),
            sample_rate: Some(number(AUDIO_SAMPLE_RATE))
                .filter(|rate| Audio::SAMPLE_RATES.contains(rate))
                .unwrap_or(fallback.audio.sample_rate),
            bitrate: Some(number(AUDIO_BITRATE))
                .filter(|bitrate| AAC_BITRATES.contains(bitrate))
                .unwrap_or(fallback.audio.bitrate),
        };
        let still = Still {
            format: named(&STILL_FORMATS, &choice(STILL_FORMAT)).unwrap_or(fallback.still.format),
            resolution: resolution(&choice(STILL_RESOLUTION)).unwrap_or(fallback.still.resolution),
        };
        Settings {
            target: named(&TARGETS, &choice(TARGET)).unwrap_or(fallback.target),
            video,
            audio,
            still,
        }
    }

    pub fn keep_export(&mut self, settings: &Settings) {
        let store = &mut self.0;
        let video = &settings.video;
        store.set(TARGET, name_of(&TARGETS, settings.target));
        store.set(CONTAINER, name_of(&CONTAINERS, video.container));
        store.set(CODEC, name_of(&CODECS, video.codec));
        store.set(ENCODER, name_of(&ENCODERS, video.encoder));
        store.set(SPEED, name_of(&SPEEDS, video.speed));
        store.set(COMPRESSION, name_of(&COMPRESSIONS, video.compression));
        store.set(QUALITY, video.quality);
        store.set(BITRATE, video.bitrate);
        store.set(RESOLUTION, resolution_name(video.resolution));
        store.set(FRAME_RATE, frame_rate_name(video.frame_rate));
        store.set(SOUND, video.sound);
        store.set(SOUND_BITRATE, video.sound_bitrate);
        let audio = &settings.audio;
        store.set(AUDIO_CODEC, name_of(&SOUND_CODECS, audio.codec));
        store.set(AUDIO_DEPTH, name_of(&DEPTHS, audio.depth));
        store.set(AUDIO_SAMPLE_RATE, audio.sample_rate);
        store.set(AUDIO_BITRATE, audio.bitrate);
        store.set(STILL_FORMAT, name_of(&STILL_FORMATS, settings.still.format));
        store.set(STILL_RESOLUTION, resolution_name(settings.still.resolution));
    }

    #[must_use]
    pub fn export_folder(&self) -> Option<PathBuf> {
        let folder = self.0.get::<String>(FOLDER);
        Some(PathBuf::from(folder)).filter(|folder| folder.is_absolute() && folder.is_dir())
    }

    pub fn keep_export_folder(&mut self, folder: &std::path::Path) {
        if let Some(folder) = folder.to_str() {
            self.0.set(FOLDER, folder);
        }
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    let settings = Settings::default();
    let video = settings.video;
    let audio = settings.audio;
    schema
        .key(choice(TARGET, &TARGETS, settings.target).summary("What File → Export makes"))
        .key(
            Key::string(FOLDER, "")
                .summary("Folder of the last export")
                .role(Role::State),
        )
        .key(choice(CONTAINER, &CONTAINERS, video.container).summary("File format of videos"))
        .key(choice(CODEC, &CODECS, video.codec).summary("Video codec"))
        .key(choice(ENCODER, &ENCODERS, video.encoder).summary("Encoder used for videos"))
        .key(choice(SPEED, &SPEEDS, video.speed).summary("Encoding speed of videos"))
        .key(
            choice(COMPRESSION, &COMPRESSIONS, video.compression)
                .summary("Whether videos hold a quality or a bitrate"),
        )
        .key(
            Key::int(QUALITY, i64::from(video.quality))
                .range(i64::from(*Video::QUALITIES.start())..=i64::from(*Video::QUALITIES.end()))
                .summary("Constant quality of videos, 1 to 100"),
        )
        .key(
            Key::int(BITRATE, i64::from(video.bitrate))
                .range(i64::from(*Video::BITRATES.start())..=i64::from(*Video::BITRATES.end()))
                .summary("Video bitrate in Mbit/s"),
        )
        .key(
            Key::string(RESOLUTION, resolution_name(video.resolution))
                .choices(resolution_names())
                .summary("Picture size of videos: project or the short side in pixels"),
        )
        .key(
            Key::string(FRAME_RATE, frame_rate_name(video.frame_rate))
                .choices(FrameRate::offered().map(frame_rate_name))
                .summary("Frames per second of videos: project or a rate"),
        )
        .key(Key::bool(SOUND, video.sound).summary("Whether videos carry the sound"))
        .key(
            Key::int(SOUND_BITRATE, i64::from(video.sound_bitrate))
                .choices(AAC_BITRATES.map(i64::from))
                .summary("AAC bitrate of videos in kbit/s"),
        )
        .key(
            choice(AUDIO_CODEC, &SOUND_CODECS, audio.codec).summary("File format of sound exports"),
        )
        .key(choice(AUDIO_DEPTH, &DEPTHS, audio.depth).summary("Bit depth of WAV exports"))
        .key(
            Key::int(AUDIO_SAMPLE_RATE, i64::from(audio.sample_rate))
                .choices(Audio::SAMPLE_RATES.map(i64::from))
                .summary("Sample rate of sound exports in Hz"),
        )
        .key(
            Key::int(AUDIO_BITRATE, i64::from(audio.bitrate))
                .choices(AAC_BITRATES.map(i64::from))
                .summary("AAC bitrate of sound exports in kbit/s"),
        )
        .key(
            choice(STILL_FORMAT, &STILL_FORMATS, settings.still.format)
                .summary("File format of frame exports"),
        )
        .key(
            Key::string(STILL_RESOLUTION, resolution_name(settings.still.resolution))
                .choices(resolution_names())
                .summary("Picture size of frame exports"),
        )
}

fn resolution_names() -> impl Iterator<Item = String> {
    std::iter::once(Resolution::Project)
        .chain(Resolution::SHORT_SIDES.map(Resolution::Short))
        .map(resolution_name)
}

fn resolution_name(resolution: Resolution) -> String {
    match resolution {
        Resolution::Project => PROJECT.to_owned(),
        Resolution::Short(side) => side.to_string(),
    }
}

fn resolution(name: &str) -> Option<Resolution> {
    match name {
        PROJECT => Some(Resolution::Project),
        side => side
            .parse()
            .ok()
            .filter(|side| Resolution::SHORT_SIDES.contains(side))
            .map(Resolution::Short),
    }
}

fn frame_rate_name(frame_rate: FrameRate) -> String {
    match frame_rate {
        FrameRate::Project => PROJECT.to_owned(),
        FrameRate::Fixed(rate) => format!("{}/{}", rate.numerator(), rate.denominator()),
    }
}

fn frame_rate(name: &str) -> Option<FrameRate> {
    if name == PROJECT {
        return Some(FrameRate::Project);
    }
    let (numerator, denominator) = name.split_once('/')?;
    let rate = Rate::new(numerator.parse().ok()?, denominator.parse().ok()?).ok()?;
    FrameRate::FIXED
        .contains(&rate)
        .then_some(FrameRate::Fixed(rate))
}
