use rikka_audio::PlaybackEvent;
use rikka_sync::{Mutex, MutexGuard};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::ffi;
use crate::media::signals::Signals;
use crate::media::voice::{Settings, Voice};
use crate::media::{check_file, clamp_to};
use crate::window::Listener;

#[derive(Debug)]
enum Phase {
    Opening { reopening: bool },
    Open(Voice),
    Failed(String),
}

#[derive(Debug)]
struct Playback {
    phase: Phase,
    playing: bool,
    ended: bool,
    start: Duration,
    duration: Option<Duration>,
    settings: Settings,
    listener: Option<Listener>,
}

impl Playback {
    fn finish(&mut self) -> bool {
        let Phase::Open(voice) = &self.phase else {
            return false;
        };
        if !voice.is_finished() {
            return false;
        }
        self.phase = Phase::Opening { reopening: true };
        self.playing = false;
        self.ended = true;
        self.start = Duration::ZERO;
        true
    }

    fn state(&self) -> ffi::PlaybackState {
        match (&self.phase, self.ended, self.playing) {
            (Phase::Failed(_), _, _) => ffi::PlaybackState::Failed,
            (_, true, _) => ffi::PlaybackState::Ended,
            (Phase::Opening { reopening: false }, _, _) => ffi::PlaybackState::Loading,
            (_, false, true) => ffi::PlaybackState::Playing,
            (_, false, false) => ffi::PlaybackState::Paused,
        }
    }

    fn position(&self) -> Duration {
        let position = match (&self.phase, self.ended) {
            (_, true) => self.duration.unwrap_or_default(),
            (Phase::Open(voice), false) => voice.position(),
            (Phase::Opening { .. }, false) => self.start,
            (Phase::Failed(_), false) => Duration::ZERO,
        };
        clamp_to(position, self.duration)
    }
}

#[derive(Debug)]
pub(crate) struct Sound {
    path: PathBuf,
    signals: Signals,
    playback: Mutex<Playback>,
    pinned: Mutex<Option<Arc<Sound>>>,
}

impl Sound {
    pub(crate) fn open(path: &str) -> Arc<Self> {
        let path = PathBuf::from(path);
        let checked = check_file(&path);
        let sound = Arc::new(Self {
            path,
            signals: Signals::default(),
            playback: Mutex::new(Playback {
                phase: Phase::Opening { reopening: false },
                playing: false,
                ended: false,
                start: Duration::ZERO,
                duration: None,
                settings: Settings::default(),
                listener: None,
            }),
            pinned: Mutex::new(None),
        });
        match checked {
            Ok(()) => sound.load(),
            Err(failure) => sound.fail(failure),
        }
        sound
    }

    fn lock(&self) -> MutexGuard<'_, Playback> {
        self.playback.lock()
    }

    fn load(self: &Arc<Self>) {
        let settings = self.lock().settings;
        if let Err(failure) = Voice::open_for(self, self.path.clone(), settings, Self::opened) {
            self.fail(failure);
        }
    }

    fn fail(&self, failure: String) {
        self.let_go();
        let mut playback = self.lock();
        playback.phase = Phase::Failed(failure);
        playback.playing = false;
        playback.ended = false;
        drop(playback);
        self.signals.changed();
    }

    fn opened(self: &Arc<Self>, opened: rikka_audio::Result<Voice>) {
        let mut voice = match opened {
            Ok(voice) => voice,
            Err(error) => {
                return self.fail(format!("failed to play {}: {error}", self.path.display()));
            }
        };
        let heard = Arc::downgrade(self);
        voice.watch(move |event| {
            if let Some(sound) = heard.upgrade() {
                sound.heard(event);
            }
        });
        let playing = {
            let mut playback = self.lock();
            voice.apply(playback.settings);
            if playback.start > Duration::ZERO {
                voice.seek(playback.start);
            }
            if playback.playing {
                voice.play();
            }
            playback.duration = voice.duration().or(playback.duration);
            playback.phase = Phase::Open(voice);
            playback.playing
        };
        self.signals.changed();
        if !playing {
            self.let_go();
        }
    }

    fn heard(self: &Arc<Self>, event: PlaybackEvent) {
        match event {
            PlaybackEvent::Progressed => self.signals.position_changed(),
            PlaybackEvent::Finished => self.update(|_| ()),
        }
    }

    fn update<R>(self: &Arc<Self>, change: impl FnOnce(&mut Playback) -> R) -> R {
        let (result, ended) = {
            let mut playback = self.lock();
            let ended = playback.finish().then(|| playback.listener.clone());
            (change(&mut playback), ended)
        };
        if let Some(listener) = ended {
            self.let_go();
            self.signals.changed();
            self.load();
            if let Some(listener) = listener {
                listener.notify();
            }
        }
        result
    }

    fn hold(self: &Arc<Self>) {
        *self.pinned.lock() = Some(Arc::clone(self));
    }

    fn let_go(&self) {
        let released = self.pinned.lock().take();
        drop(released);
    }

    pub(crate) fn play(self: &Arc<Self>) {
        let started = self.update(|playback| {
            if matches!(playback.phase, Phase::Failed(_)) {
                return false;
            }
            if let Phase::Open(voice) = &playback.phase {
                voice.play();
            }
            playback.playing = true;
            playback.ended = false;
            true
        });
        if started {
            self.hold();
            self.signals.state_changed();
        }
    }

    pub(crate) fn pause(self: &Arc<Self>) {
        let paused = self.update(|playback| {
            if let Phase::Open(voice) = &playback.phase {
                voice.pause();
            }
            std::mem::replace(&mut playback.playing, false)
        });
        self.let_go();
        if paused {
            self.signals.state_changed();
        }
    }

    pub(crate) fn stop(self: &Arc<Self>) {
        self.update(|playback| {
            if let Phase::Open(voice) = &playback.phase {
                voice.pause();
                voice.seek(Duration::ZERO);
            }
            playback.playing = false;
            playback.ended = false;
            playback.start = Duration::ZERO;
        });
        self.let_go();
        self.signals.changed();
    }

    pub(crate) fn seek(self: &Arc<Self>, at: Duration) {
        self.update(|playback| {
            let at = clamp_to(at, playback.duration);
            if let Phase::Open(voice) = &playback.phase {
                voice.seek(at);
            }
            playback.ended = false;
            playback.start = at;
        });
        self.signals.changed();
    }

    pub(crate) fn on_end(&self, listener: Listener) {
        self.lock().listener = Some(listener);
    }

    pub(crate) fn state(&self) -> ffi::PlaybackState {
        self.signals.read_state();
        self.lock().state()
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.signals.read_state();
        self.lock().playing
    }

    pub(crate) fn position(&self) -> Duration {
        self.signals.read_position();
        self.lock().position()
    }

    pub(crate) fn duration(&self) -> Option<Duration> {
        self.signals.read_state();
        self.lock().duration
    }

    pub(crate) fn settings(&self) -> Settings {
        self.signals.read_state();
        self.lock().settings
    }

    pub(crate) fn set_settings(&self, change: impl FnOnce(&mut Settings)) {
        let mut playback = self.lock();
        let before = playback.settings;
        change(&mut playback.settings);
        if playback.settings == before {
            return;
        }
        if let Phase::Open(voice) = &playback.phase {
            voice.apply(playback.settings);
        }
        drop(playback);
        self.signals.state_changed();
    }

    pub(crate) fn error_message(&self) -> String {
        self.signals.read_state();
        match &self.lock().phase {
            Phase::Failed(failure) => failure.clone(),
            Phase::Opening { .. } | Phase::Open(_) => String::new(),
        }
    }
}
