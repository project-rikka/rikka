use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_channel::mpsc::UnboundedSender;
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedValue};

use crate::metadata;
use crate::publish::playback::{Command, PlayerState};
use crate::track::Repeat;

pub(crate) type Shared = Arc<Mutex<PlayerState>>;

pub(crate) fn owned_metadata(state: &PlayerState) -> HashMap<String, OwnedValue> {
    metadata::encode(state.playback.track.as_ref())
        .into_iter()
        .filter_map(|(key, value)| Some((key, OwnedValue::try_from(value).ok()?)))
        .collect()
}

pub(crate) fn micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

pub(crate) struct Player {
    pub(crate) shared: Shared,
    pub(crate) commands: UnboundedSender<Command>,
}

impl Player {
    fn send(&self, command: Command) {
        let _ = self.commands.unbounded_send(command);
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) {
        if self.shared.lock().playback.abilities.next {
            self.send(Command::Next);
        }
    }

    fn previous(&self) {
        if self.shared.lock().playback.abilities.previous {
            self.send(Command::Previous);
        }
    }

    fn pause(&self) {
        if self.shared.lock().playback.abilities.pause {
            self.send(Command::Pause);
        }
    }

    fn play_pause(&self) {
        let abilities = self.shared.lock().playback.abilities;
        if abilities.play || abilities.pause {
            self.send(Command::PlayPause);
        }
    }

    fn stop(&self) {
        if self.shared.lock().playback.abilities.control {
            self.send(Command::Stop);
        }
    }

    fn play(&self) {
        if self.shared.lock().playback.abilities.play {
            self.send(Command::Play);
        }
    }

    fn seek(&self, offset: i64) {
        if !self.shared.lock().playback.abilities.seek {
            return;
        }
        self.send(Command::SeekBy {
            forward: offset >= 0,
            by: Duration::from_micros(offset.unsigned_abs()),
        });
    }

    fn set_position(&self, track: ObjectPath<'_>, position: i64) {
        let state = self.shared.lock();
        let Some(current) = &state.playback.track else {
            return;
        };
        if !state.playback.abilities.seek || metadata::track_path(&current.id) != track.as_str() {
            return;
        }
        let Ok(position) = u64::try_from(position) else {
            return;
        };
        let position = Duration::from_micros(position);
        if current.length.is_some_and(|length| position > length) {
            return;
        }
        drop(state);
        self.send(Command::SeekTo(position));
    }

    fn open_uri(&self, _uri: String) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::NotSupported(
            "this player opens nothing it is handed".into(),
        ))
    }

    #[zbus(signal)]
    pub(crate) async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> String {
        self.shared.lock().playback.status.name().to_owned()
    }

    #[zbus(property)]
    fn loop_status(&self) -> String {
        self.shared
            .lock()
            .playback
            .repeat
            .unwrap_or_default()
            .name()
            .to_owned()
    }

    #[zbus(property)]
    fn set_loop_status(&self, named: String) {
        let Some(repeat) = Repeat::named(&named) else {
            return;
        };
        let mut state = self.shared.lock();
        if state.playback.repeat.is_none() || !state.playback.abilities.control {
            return;
        }
        state.playback.repeat = Some(repeat);
        drop(state);
        self.send(Command::Repeat(repeat));
    }

    #[zbus(property)]
    fn rate(&self) -> f64 {
        self.shared.lock().playback.rate
    }

    #[zbus(property)]
    fn set_rate(&self, _rate: f64) {}

    #[zbus(property)]
    fn shuffle(&self) -> bool {
        self.shared.lock().playback.shuffle.unwrap_or(false)
    }

    #[zbus(property)]
    fn set_shuffle(&self, shuffle: bool) {
        let mut state = self.shared.lock();
        if state.playback.shuffle.is_none() || !state.playback.abilities.control {
            return;
        }
        state.playback.shuffle = Some(shuffle);
        drop(state);
        self.send(Command::Shuffle(shuffle));
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        owned_metadata(&self.shared.lock())
    }

    #[zbus(property)]
    fn volume(&self) -> f64 {
        self.shared.lock().playback.volume
    }

    #[zbus(property)]
    fn set_volume(&self, volume: f64) {
        let mut state = self.shared.lock();
        if !state.playback.abilities.control || !volume.is_finite() {
            return;
        }
        let volume = volume.max(0.0);
        state.playback.volume = volume;
        drop(state);
        self.send(Command::Volume(volume));
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        micros(self.shared.lock().position(Instant::now()))
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn minimum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn maximum_rate(&self) -> f64 {
        1.0
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        self.shared.lock().playback.abilities.next
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        self.shared.lock().playback.abilities.previous
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        self.shared.lock().playback.abilities.play
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        self.shared.lock().playback.abilities.pause
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        self.shared.lock().playback.abilities.seek
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        self.shared.lock().playback.abilities.control
    }
}
