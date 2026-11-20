use music_core::Quality;
use rikka_hifi::{Output, ReplayGain, ReplayGainMode};
use rikka_settings::{Key, Schema, Value};
use rikka_types::Choice;

use crate::cache_size::CacheSize;

const DOMAIN: &str = "rikka.music";
const AUDIO: &str = "audio-quality";
const IMAGE: &str = "image-quality";
const CACHE: &str = "audio-cache";
const ORDER: &str = "service-order";
const OUTPUT: &str = "output";
const REPLAY_GAIN: &str = "replay-gain";
const PREAMP: &str = "replay-gain-preamp";
const PREVENT_CLIPPING: &str = "prevent-clipping";

const SHARED: &str = "shared";
const SINK: &str = "sink:";
pub(crate) const PREAMP_DB: f32 = 15.0;
pub(crate) const GAIN_MODES: [ReplayGainMode; 3] = [
    ReplayGainMode::Off,
    ReplayGainMode::Track,
    ReplayGainMode::Album,
];

#[derive(Debug)]
pub(crate) struct Preferences(rikka_settings::seam::Preferences);

impl Preferences {
    pub(crate) fn open() -> Preferences {
        Preferences(rikka_settings::seam::Preferences::open(schema()))
    }

    pub(crate) fn audio(&self) -> Quality {
        self.quality(AUDIO)
    }

    pub(crate) fn image(&self) -> Quality {
        self.quality(IMAGE)
    }

    pub(crate) fn cache(&self) -> CacheSize {
        CacheSize::of_slug(&self.0.get::<String>(CACHE))
    }

    pub(crate) fn order(&self) -> Vec<String> {
        self.0.get::<Vec<String>>(ORDER)
    }

    pub(crate) fn output(&self) -> Output {
        output_of(&self.0.get::<String>(OUTPUT))
    }

    pub(crate) fn replay_gain(&self) -> ReplayGain {
        let message = self.0.get::<String>(REPLAY_GAIN);
        let mode = GAIN_MODES
            .into_iter()
            .find(|mode| gain_slug(*mode) == message)
            .unwrap_or(ReplayGainMode::Off);
        ReplayGain::new(mode)
            .preamp(self.0.get::<f64>(PREAMP) as f32)
            .prevent_clipping(self.0.get::<bool>(PREVENT_CLIPPING))
    }

    pub(crate) fn set_order(&mut self, order: &[String]) {
        self.0.set(ORDER, order);
    }

    pub(crate) fn set_audio(&mut self, quality: Quality) {
        self.0.set(AUDIO, quality.slug());
    }

    pub(crate) fn set_image(&mut self, quality: Quality) {
        self.0.set(IMAGE, quality.slug());
    }

    pub(crate) fn set_cache(&mut self, size: CacheSize) {
        self.0.set(CACHE, size.slug());
    }

    pub(crate) fn set_output(&mut self, output: &Output) {
        self.0.set(OUTPUT, slug_of(output));
    }

    pub(crate) fn set_replay_gain(&mut self, replay_gain: ReplayGain) {
        self.0.set(REPLAY_GAIN, gain_slug(replay_gain.mode));
        self.0.set(
            PREAMP,
            f64::from(replay_gain.preamp_db.clamp(-PREAMP_DB, PREAMP_DB)),
        );
        self.0.set(PREVENT_CLIPPING, replay_gain.prevent_clipping);
    }

    fn quality(&self, key: &str) -> Quality {
        Quality::from_slug(&self.0.get::<String>(key)).unwrap_or_default()
    }
}

impl Default for Preferences {
    fn default() -> Preferences {
        Preferences(rikka_settings::seam::Preferences::in_memory(schema()))
    }
}

fn output_of(slug: &str) -> Output {
    if let Some(sink) = slug.strip_prefix(SINK).filter(|sink| !sink.is_empty()) {
        return Output::Sink(sink.to_owned());
    }
    slug.parse().map_or(Output::Shared, Output::Exclusive)
}

fn slug_of(output: &Output) -> String {
    match output {
        Output::Shared => SHARED.to_owned(),
        Output::Sink(sink) => format!("{SINK}{sink}"),
        Output::Exclusive(device) => device.pcm(),
    }
}

fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("Music")
        .key(quality_key(AUDIO).summary("Streaming audio quality"))
        .key(quality_key(IMAGE).summary("Artwork quality"))
        .key(Key::choice::<CacheSize>(CACHE).summary("Played music cache size"))
        .key(
            Key::list(ORDER, std::iter::empty::<Value>())
                .summary("Services tried first for songs another service cannot play"),
        )
        .key(Key::string(OUTPUT, SHARED).summary(
            "Where music plays: shared, sink:<PipeWire sink> or an exclusive hw:CARD=<card>,DEV=<n>",
        ))
        .key(
            Key::string(REPLAY_GAIN, gain_slug(ReplayGainMode::Off))
                .choices(GAIN_MODES.map(gain_slug))
                .summary("ReplayGain loudness levelling"),
        )
        .key(
            Key::float(PREAMP, 0.0)
                .range(-f64::from(PREAMP_DB)..=f64::from(PREAMP_DB))
                .summary("ReplayGain preamp in dB"),
        )
        .key(
            Key::bool(PREVENT_CLIPPING, true)
                .summary("Lower ReplayGain so the peak never clips"),
        )
}

fn gain_slug(mode: ReplayGainMode) -> &'static str {
    match mode {
        ReplayGainMode::Off => "off",
        ReplayGainMode::Track => "track",
        ReplayGainMode::Album => "album",
    }
}

fn quality_key(name: &str) -> Key {
    Key::string(name, Quality::default().slug()).choices(Quality::ALL.map(Quality::slug))
}
