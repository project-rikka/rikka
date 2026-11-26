use animate_cast::{LARGEST_SIDE, Resolution, SMALLEST_SIDE};
use animate_export::{
    BITRATES, CODECS, CONTAINERS, ENCODERS, Encoding, FRAME_RATES, QUALITIES, RATES,
    SOUND_BITRATES, SPEEDS, name_of, named,
};
use rikka_settings::{Key, Schema};

use super::Preferences;

const WIDTH: &str = "output-width";
const HEIGHT: &str = "output-height";
const FRAME_RATE: &str = "export-frame-rate";
const CONTAINER: &str = "export-container";
const CODEC: &str = "export-codec";
const ENCODER: &str = "export-encoder";
const SPEED: &str = "export-speed";
const RATE: &str = "export-rate";
const QUALITY: &str = "export-quality";
const BITRATE: &str = "export-bitrate";
const SOUND: &str = "export-sound";
const SOUND_BITRATE: &str = "export-sound-bitrate";

impl Preferences {
    pub fn resolution(&self) -> Resolution {
        let settings = self.0.borrow();
        Resolution::new(settings.get(WIDTH), settings.get(HEIGHT)).unwrap_or_default()
    }

    pub fn keep_resolution(&self, resolution: Resolution) {
        let mut settings = self.0.borrow_mut();
        settings.set(WIDTH, resolution.width());
        settings.set(HEIGHT, resolution.height());
    }

    pub fn encoding(&self) -> Encoding {
        let settings = self.0.borrow();
        let fallback = Encoding::default();
        let choice = |key: &str| settings.get::<String>(key);
        Encoding {
            frame_rate: named(&FRAME_RATES, &choice(FRAME_RATE)).unwrap_or(fallback.frame_rate),
            container: named(&CONTAINERS, &choice(CONTAINER)).unwrap_or(fallback.container),
            codec: named(&CODECS, &choice(CODEC)).unwrap_or(fallback.codec),
            encoder: named(&ENCODERS, &choice(ENCODER)).unwrap_or(fallback.encoder),
            speed: named(&SPEEDS, &choice(SPEED)).unwrap_or(fallback.speed),
            rate: named(&RATES, &choice(RATE)).unwrap_or(fallback.rate),
            quality: u8::try_from(settings.get::<u32>(QUALITY))
                .ok()
                .filter(|quality| QUALITIES.contains(quality))
                .unwrap_or(fallback.quality),
            bitrate: Some(settings.get::<u32>(BITRATE))
                .filter(|bitrate| BITRATES.contains(bitrate))
                .unwrap_or(fallback.bitrate),
            sound: settings.get(SOUND),
            sound_bitrate: Some(settings.get::<u32>(SOUND_BITRATE))
                .filter(|bitrate| SOUND_BITRATES.contains(bitrate))
                .unwrap_or(fallback.sound_bitrate),
        }
    }

    pub fn keep_encoding(&self, encoding: &Encoding) {
        let mut settings = self.0.borrow_mut();
        settings.set(FRAME_RATE, name_of(&FRAME_RATES, encoding.frame_rate));
        settings.set(CONTAINER, name_of(&CONTAINERS, encoding.container));
        settings.set(CODEC, name_of(&CODECS, encoding.codec));
        settings.set(ENCODER, name_of(&ENCODERS, encoding.encoder));
        settings.set(SPEED, name_of(&SPEEDS, encoding.speed));
        settings.set(RATE, name_of(&RATES, encoding.rate));
        settings.set(QUALITY, encoding.quality);
        settings.set(BITRATE, encoding.bitrate);
        settings.set(SOUND, encoding.sound);
        settings.set(SOUND_BITRATE, encoding.sound_bitrate);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    let default = Resolution::default();
    let encoding = Encoding::default();
    schema
        .key(side(WIDTH, default.width()).summary("Output width in pixels"))
        .key(side(HEIGHT, default.height()).summary("Output height in pixels"))
        .key(
            choice(
                FRAME_RATE,
                &FRAME_RATES,
                name_of(&FRAME_RATES, encoding.frame_rate),
            )
            .summary("Frames per second of exported videos"),
        )
        .key(
            choice(
                CONTAINER,
                &CONTAINERS,
                name_of(&CONTAINERS, encoding.container),
            )
            .summary("File format of exported videos"),
        )
        .key(
            choice(CODEC, &CODECS, name_of(&CODECS, encoding.codec))
                .summary("Video codec of exported videos"),
        )
        .key(
            choice(ENCODER, &ENCODERS, name_of(&ENCODERS, encoding.encoder))
                .summary("Encoder used to export videos"),
        )
        .key(
            choice(SPEED, &SPEEDS, name_of(&SPEEDS, encoding.speed))
                .summary("Encoding speed of exported videos"),
        )
        .key(
            choice(RATE, &RATES, name_of(&RATES, encoding.rate))
                .summary("Whether exports hold a quality or a bitrate"),
        )
        .key(
            Key::int(QUALITY, i64::from(encoding.quality))
                .range(i64::from(*QUALITIES.start())..=i64::from(*QUALITIES.end()))
                .summary("Constant quality of exported videos, 1 to 100"),
        )
        .key(
            Key::int(BITRATE, i64::from(encoding.bitrate))
                .range(i64::from(*BITRATES.start())..=i64::from(*BITRATES.end()))
                .summary("Video bitrate of exported videos in Mbit/s"),
        )
        .key(Key::bool(SOUND, encoding.sound).summary("Whether exported videos carry the music"))
        .key(
            Key::int(SOUND_BITRATE, i64::from(encoding.sound_bitrate))
                .choices(SOUND_BITRATES.map(i64::from))
                .summary("Audio bitrate of exported videos in kbit/s"),
        )
}

fn side(name: &str, default: u32) -> Key {
    Key::int(name, i64::from(default)).range(i64::from(SMALLEST_SIDE)..=i64::from(LARGEST_SIDE))
}

fn choice<T>(name: &str, table: &[(T, &str)], default: &str) -> Key {
    Key::string(name, default).choices(table.iter().map(|(_, value)| (*value).to_owned()))
}
