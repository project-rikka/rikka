use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use discord_core::{ChannelId, UserId};
use discord_stream::{FrameRate, Quality, Resolution};
use discord_voice::{InputMode, MAX_VOLUME, NoiseSuppression, VoiceSettings};
use rikka_app::prelude::Key as Keyboard;
use rikka_settings::{Key, Schema, Values};

type PerAccount<T> = BTreeMap<String, T>;

use crate::voice::hotkey;

const DOMAIN: &str = "rikka.discord";
const MUTED: &str = "muted";
const DEAFENED: &str = "deafened";
const COLLAPSED: &str = "collapsed-categories";
const INPUT: &str = "input-device";
const OUTPUT: &str = "output-device";
const INPUT_VOLUME: &str = "input-volume";
const OUTPUT_VOLUME: &str = "output-volume";
const MODE: &str = "input-mode";
const SENSITIVITY: &str = "sensitivity";
const AUTOMATIC: &str = "automatic-sensitivity";
const TALK_KEY: &str = "push-to-talk-key";
const TALK_RELEASE: &str = "push-to-talk-release";
const SUPPRESSION: &str = "noise-suppression-kind";
const ECHO: &str = "echo-cancellation";
const GAIN: &str = "automatic-gain";
const USER_VOLUMES: &str = "user-volumes";
const NOTIFICATIONS: &str = "notifications";
const NOTIFY_DIRECTS: &str = "notify-directs";
const NOTIFY_MENTIONS: &str = "notify-mentions";
const DISPLAY: &str = "message-display";
const EMBEDS: &str = "show-embeds";
const ANIMATED: &str = "animated-emoji";
const STREAM_RESOLUTION: &str = "stream-resolution";
const STREAM_FRAME_RATE: &str = "stream-frame-rate";
const STREAM_SOUND: &str = "stream-sound";

const VOICE_ACTIVITY: &str = "voice-activity";
const PUSH_TO_TALK: &str = "push-to-talk";
const COZY: &str = "cozy";
const COMPACT: &str = "compact";
const DEFAULT_SENSITIVITY: f64 = -50.0;
const DEFAULT_RELEASE_MS: i64 = 200;
const DEFAULT_TALK_KEY: Keyboard = Keyboard::F(8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Appearance {
    pub(crate) compact: bool,
    pub(crate) embeds: bool,
    pub(crate) animated_emoji: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Alerts {
    pub(crate) enabled: bool,
    pub(crate) directs: bool,
    pub(crate) mentions: bool,
}

#[derive(Debug)]
pub(crate) struct Preferences(rikka_settings::seam::Preferences);

fn resolution_named(name: &str) -> Resolution {
    Resolution::ALL
        .into_iter()
        .find(|resolution| resolution_name(*resolution) == name)
        .unwrap_or_default()
}

fn resolution_name(resolution: Resolution) -> &'static str {
    match resolution {
        Resolution::Hd => "720p",
        Resolution::FullHd => "1080p",
        Resolution::QuadHd => "1440p",
        Resolution::UltraHd => "2160p",
        Resolution::Source => "source",
    }
}

fn suppression_of(name: &str) -> NoiseSuppression {
    NoiseSuppression::ALL
        .into_iter()
        .find(|kind| suppression_name(*kind) == name)
        .unwrap_or_default()
}

fn suppression_name(kind: NoiseSuppression) -> &'static str {
    match kind {
        NoiseSuppression::Off => "off",
        NoiseSuppression::Standard => "standard",
        NoiseSuppression::Krisp => "krisp",
    }
}

impl Preferences {
    pub(crate) fn open() -> Preferences {
        Preferences(rikka_settings::seam::Preferences::open(schema()))
    }

    pub(crate) fn muted(&self) -> bool {
        self.0.get::<bool>(MUTED)
    }

    pub(crate) fn deafened(&self) -> bool {
        self.0.get::<bool>(DEAFENED)
    }

    pub(crate) fn set_muted(&mut self, muted: bool) {
        self.0.set(MUTED, muted);
    }

    pub(crate) fn set_deafened(&mut self, deafened: bool) {
        self.0.set(DEAFENED, deafened);
    }

    pub(crate) fn collapsed_in(&self, account: &str) -> BTreeSet<ChannelId> {
        self.0
            .get::<PerAccount<BTreeSet<ChannelId>>>(COLLAPSED)
            .remove(account)
            .unwrap_or_default()
    }

    pub(crate) fn set_collapsed(&mut self, account: &str, category: &ChannelId, collapsed: bool) {
        let mut per_account = self.0.get::<PerAccount<BTreeSet<ChannelId>>>(COLLAPSED);
        let folded = per_account.entry(account.to_owned()).or_default();
        match collapsed {
            true => folded.insert(*category),
            false => folded.remove(category),
        };
        if folded.is_empty() {
            per_account.remove(account);
        }
        self.0.set(COLLAPSED, per_account);
    }

    pub(crate) fn forget_account(&mut self, account: &str) {
        let mut folded = self.0.get::<PerAccount<BTreeSet<ChannelId>>>(COLLAPSED);
        if folded.remove(account).is_some() {
            self.0.set(COLLAPSED, folded);
        }
        let mut volumes = self
            .0
            .get::<PerAccount<BTreeMap<UserId, f32>>>(USER_VOLUMES);
        if volumes.remove(account).is_some() {
            self.0.set(USER_VOLUMES, volumes);
        }
    }

    pub(crate) fn voice(&self) -> VoiceSettings {
        let mode = match self.0.get::<String>(MODE).as_str() {
            PUSH_TO_TALK => InputMode::PushToTalk {
                release: Duration::from_millis(
                    u64::try_from(self.0.get::<i64>(TALK_RELEASE)).unwrap_or_default(),
                ),
            },
            _ => InputMode::VoiceActivity {
                threshold_db: self.0.get::<f64>(SENSITIVITY) as f32,
                automatic: self.0.get::<bool>(AUTOMATIC),
            },
        };
        VoiceSettings {
            input: device(self.0.get::<String>(INPUT)),
            output: device(self.0.get::<String>(OUTPUT)),
            input_volume: self.0.get::<f64>(INPUT_VOLUME) as f32,
            output_volume: self.0.get::<f64>(OUTPUT_VOLUME) as f32,
            mode,
            noise_suppression: suppression_of(&self.0.get::<String>(SUPPRESSION)),
            echo_cancellation: self.0.get::<bool>(ECHO),
            automatic_gain: self.0.get::<bool>(GAIN),
        }
    }

    pub(crate) fn set_voice(&mut self, settings: &VoiceSettings) {
        self.0
            .set(INPUT, settings.input.clone().unwrap_or_default());
        self.0
            .set(OUTPUT, settings.output.clone().unwrap_or_default());
        self.0.set(INPUT_VOLUME, f64::from(settings.input_volume));
        self.0.set(OUTPUT_VOLUME, f64::from(settings.output_volume));
        self.0
            .set(SUPPRESSION, suppression_name(settings.noise_suppression));
        self.0.set(ECHO, settings.echo_cancellation);
        self.0.set(GAIN, settings.automatic_gain);
        match settings.mode {
            InputMode::VoiceActivity {
                threshold_db,
                automatic,
            } => {
                self.0.set(MODE, VOICE_ACTIVITY);
                self.0.set(SENSITIVITY, f64::from(threshold_db));
                self.0.set(AUTOMATIC, automatic);
            }
            InputMode::PushToTalk { release } => {
                self.0.set(MODE, PUSH_TO_TALK);
                self.0.set(
                    TALK_RELEASE,
                    i64::try_from(release.as_millis()).unwrap_or(DEFAULT_RELEASE_MS),
                );
            }
        }
    }

    pub(crate) fn talk_key(&self) -> Keyboard {
        hotkey::decode(&self.0.get::<String>(TALK_KEY)).unwrap_or(DEFAULT_TALK_KEY)
    }

    pub(crate) fn set_talk_key(&mut self, key: Keyboard) {
        if let Some(encoded) = hotkey::encode(key) {
            self.0.set(TALK_KEY, encoded);
        }
    }

    pub(crate) fn user_volume(&self, account: &str, user: &UserId) -> f32 {
        self.user_volumes(account).get(user).copied().unwrap_or(1.0)
    }

    pub(crate) fn user_volumes(&self, account: &str) -> BTreeMap<UserId, f32> {
        self.0
            .get::<PerAccount<BTreeMap<UserId, f32>>>(USER_VOLUMES)
            .remove(account)
            .unwrap_or_default()
    }

    pub(crate) fn set_user_volume(&mut self, account: &str, user: &UserId, volume: f32) {
        let mut per_account = self
            .0
            .get::<PerAccount<BTreeMap<UserId, f32>>>(USER_VOLUMES);
        let tuned = per_account.entry(account.to_owned()).or_default();
        let volume = volume.clamp(0.0, MAX_VOLUME);
        match (volume - 1.0).abs() > f32::EPSILON {
            true => tuned.insert(*user, volume),
            false => tuned.remove(user),
        };
        if tuned.is_empty() {
            per_account.remove(account);
        }
        self.0.set(USER_VOLUMES, per_account);
    }

    pub(crate) fn stream_quality(&self) -> Quality {
        let frame_rate = FrameRate::ALL
            .into_iter()
            .find(|rate| i64::from(rate.per_second()) == self.0.get::<i64>(STREAM_FRAME_RATE))
            .unwrap_or_default();
        Quality {
            resolution: resolution_named(&self.0.get::<String>(STREAM_RESOLUTION)),
            frame_rate,
        }
    }

    pub(crate) fn set_stream_quality(&mut self, quality: Quality) {
        self.0
            .set(STREAM_RESOLUTION, resolution_name(quality.resolution));
        self.0.set(
            STREAM_FRAME_RATE,
            i64::from(quality.frame_rate.per_second()),
        );
    }

    pub(crate) fn stream_sound(&self) -> bool {
        self.0.get::<bool>(STREAM_SOUND)
    }

    pub(crate) fn set_stream_sound(&mut self, everything: bool) {
        self.0.set(STREAM_SOUND, everything);
    }

    pub(crate) fn alerts(&self) -> Alerts {
        Alerts {
            enabled: self.0.get::<bool>(NOTIFICATIONS),
            directs: self.0.get::<bool>(NOTIFY_DIRECTS),
            mentions: self.0.get::<bool>(NOTIFY_MENTIONS),
        }
    }

    pub(crate) fn set_alerts(&mut self, alerts: Alerts) {
        self.0.set(NOTIFICATIONS, alerts.enabled);
        self.0.set(NOTIFY_DIRECTS, alerts.directs);
        self.0.set(NOTIFY_MENTIONS, alerts.mentions);
    }

    pub(crate) fn appearance(&self) -> Appearance {
        Appearance {
            compact: self.0.get::<String>(DISPLAY) == COMPACT,
            embeds: self.0.get::<bool>(EMBEDS),
            animated_emoji: self.0.get::<bool>(ANIMATED),
        }
    }

    pub(crate) fn set_appearance(&mut self, appearance: Appearance) {
        let display = match appearance.compact {
            true => COMPACT,
            false => COZY,
        };
        self.0.set(DISPLAY, display);
        self.0.set(EMBEDS, appearance.embeds);
        self.0.set(ANIMATED, appearance.animated_emoji);
    }
}

impl Default for Preferences {
    fn default() -> Preferences {
        Preferences(rikka_settings::seam::Preferences::in_memory(schema()))
    }
}

fn device(name: String) -> Option<String> {
    (!name.is_empty()).then_some(name)
}

fn schema() -> Schema {
    let volume = || f64::from(MAX_VOLUME);
    Schema::new(DOMAIN)
        .summary("Chat")
        .key(Key::bool(MUTED, false).summary("Keep the microphone muted"))
        .key(Key::bool(DEAFENED, false).summary("Keep other people silenced"))
        .key(
            Key::table(COLLAPSED, Values::new())
                .summary("Channel categories folded in the sidebar, per account"),
        )
        .key(Key::string(INPUT, "").summary("Microphone, empty for the default"))
        .key(Key::string(OUTPUT, "").summary("Speaker, empty for the default"))
        .key(
            Key::float(INPUT_VOLUME, 1.0)
                .range(0.0..=volume())
                .summary("Microphone volume"),
        )
        .key(
            Key::float(OUTPUT_VOLUME, 1.0)
                .range(0.0..=volume())
                .summary("Call volume"),
        )
        .key(
            Key::string(MODE, VOICE_ACTIVITY)
                .choices([VOICE_ACTIVITY, PUSH_TO_TALK])
                .summary("How the microphone opens"),
        )
        .key(
            Key::float(SENSITIVITY, DEFAULT_SENSITIVITY)
                .range(-100.0..=0.0)
                .summary("Voice activity threshold in dB"),
        )
        .key(Key::bool(AUTOMATIC, true).summary("Find the voice activity threshold by itself"))
        .key(
            Key::string(
                TALK_KEY,
                hotkey::encode(DEFAULT_TALK_KEY).unwrap_or_default(),
            )
            .summary("Key held to talk"),
        )
        .key(
            Key::int(TALK_RELEASE, DEFAULT_RELEASE_MS)
                .range(0..=2000)
                .summary("Milliseconds the microphone stays open after the key is let go"),
        )
        .key(
            Key::string(SUPPRESSION, suppression_name(NoiseSuppression::default()))
                .choices(NoiseSuppression::ALL.map(suppression_name))
                .summary("Noise suppression: off, standard (WebRTC) or Krisp"),
        )
        .key(Key::bool(ECHO, true).summary("Cancel the echo of the speakers in the microphone"))
        .key(Key::bool(GAIN, true).summary("Keep the microphone at a steady loudness"))
        .key(
            Key::table(USER_VOLUMES, Values::new())
                .summary("Volume of other people in calls, per account"),
        )
        .key(Key::bool(NOTIFICATIONS, true).summary("Show desktop notifications"))
        .key(Key::bool(NOTIFY_DIRECTS, true).summary("Notify about direct messages"))
        .key(Key::bool(NOTIFY_MENTIONS, true).summary("Notify about mentions in spaces"))
        .key(
            Key::string(DISPLAY, COZY)
                .choices([COZY, COMPACT])
                .summary("Message display"),
        )
        .key(Key::bool(EMBEDS, true).summary("Show embeds and link previews"))
        .key(Key::bool(ANIMATED, true).summary("Play animated emoji"))
        .key(
            Key::string(STREAM_RESOLUTION, resolution_name(Resolution::default()))
                .choices(Resolution::ALL.map(resolution_name))
                .summary("Picture size of a shared screen"),
        )
        .key(
            Key::int(
                STREAM_FRAME_RATE,
                i64::from(FrameRate::default().per_second()),
            )
            .range(15..=60)
            .summary("Frames per second of a shared screen"),
        )
        .key(
            Key::bool(STREAM_SOUND, false)
                .summary("Whether a shared screen carries the whole desktop's sound"),
        )
}
