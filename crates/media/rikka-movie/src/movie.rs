use rikka_sync::{Mutex, MutexGuard};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::thread;
use std::time::Duration;

use rikka_picture::{VideoController, VideoEvent, VideoState, VideoWatch};

use crate::error::Error;
use crate::sync::{has_arrived, has_drifted};
use crate::voice::Voice;

const OPENER: &str = "rikka-movie-sound";

type Listener = dyn Fn(MovieEvent) + Send + Sync;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MovieEvent {
    Changed,
    Progressed,
    Ended,
}

#[derive(Clone, Debug)]
pub enum Soundtrack {
    Opening,
    Open,
    Silent,
    Unplayable(Arc<Error>),
}

#[derive(Clone)]
pub struct Movie(Arc<Shared>);

struct Shared {
    path: PathBuf,
    video: VideoController,
    playback: Mutex<Playback>,
    listener: Mutex<Option<Arc<Listener>>>,
    _watch: VideoWatch,
}

struct Playback {
    playing: bool,
    ended: bool,
    target: Option<Duration>,
    sound: Sound,
    volume: f32,
    looping: bool,
}

enum Sound {
    Opening,
    Open(Voice),
    Silent,
    Unplayable(Arc<Error>),
}

impl Sound {
    fn voice(&self) -> Option<&Voice> {
        match self {
            Sound::Open(voice) => Some(voice),
            Sound::Opening | Sound::Silent | Sound::Unplayable(_) => None,
        }
    }
}

impl Movie {
    #[must_use]
    pub fn open(path: impl Into<PathBuf>) -> Movie {
        let path = path.into();
        let video = VideoController::open(&path);
        let shared = Arc::new_cyclic(|shared: &Weak<Shared>| {
            let heard = shared.clone();
            let watch = video.watch(move |event| {
                if let Some(shared) = heard.upgrade() {
                    shared.heard(event);
                }
            });
            Shared {
                path,
                video,
                playback: Mutex::new(Playback {
                    playing: false,
                    ended: false,
                    target: None,
                    sound: Sound::Opening,
                    volume: 1.0,
                    looping: false,
                }),
                listener: Mutex::new(None),
                _watch: watch,
            }
        });
        shared.reopen(&mut shared.lock());
        Movie(shared)
    }

    pub fn on_change(&self, listener: impl Fn(MovieEvent) + Send + Sync + 'static) {
        *self.0.listener.lock() = Some(Arc::new(listener));
    }

    #[must_use]
    pub fn video(&self) -> &VideoController {
        &self.0.video
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0.path
    }

    pub fn play(&self) {
        let shared = &self.0;
        let mut playback = shared.lock();
        if playback.ended || shared.video.state() == VideoState::Ended {
            shared.move_to(&mut playback, Duration::ZERO);
        }
        shared.video.play();
        if shared.video.state() != VideoState::Opening {
            shared.resume(&mut playback);
        }
        playback.playing = true;
        drop(playback);
        shared.tell(MovieEvent::Changed);
    }

    pub fn pause(&self) {
        let shared = &self.0;
        let mut playback = shared.lock();
        shared.video.pause();
        if let Some(voice) = playback.sound.voice() {
            voice.pause();
        }
        let paused = std::mem::replace(&mut playback.playing, false);
        drop(playback);
        if paused {
            shared.tell(MovieEvent::Changed);
        }
    }

    pub fn seek(&self, at: Duration) {
        let shared = &self.0;
        let mut playback = shared.lock();
        let at = clamp_to(at, shared.duration_in(&playback));
        if playback.target == Some(at) {
            return;
        }
        shared.move_to(&mut playback, at);
        drop(playback);
        shared.tell(MovieEvent::Changed);
    }

    pub fn set_volume(&self, volume: f32) {
        let shared = &self.0;
        let volume = volume.clamp(0.0, 1.0);
        let mut playback = shared.lock();
        if playback.volume == volume {
            return;
        }
        playback.volume = volume;
        if let Some(voice) = playback.sound.voice() {
            voice.set_volume(volume);
        }
        drop(playback);
        shared.tell(MovieEvent::Changed);
    }

    pub fn set_looping(&self, looping: bool) {
        let shared = &self.0;
        let mut playback = shared.lock();
        if playback.looping == looping {
            return;
        }
        playback.looping = looping;
        shared.video.set_looping(looping);
        if let Some(voice) = playback.sound.voice() {
            voice.set_looping(looping);
        }
        drop(playback);
        shared.tell(MovieEvent::Changed);
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.0.lock().playing
    }

    #[must_use]
    pub fn is_ended(&self) -> bool {
        self.0.lock().ended
    }

    #[must_use]
    pub fn is_seeking(&self) -> bool {
        self.0.lock().target.is_some()
    }

    #[must_use]
    pub fn position(&self) -> Duration {
        self.0.position_in(&self.0.lock())
    }

    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.0.duration_in(&self.0.lock())
    }

    #[must_use]
    pub fn picture(&self) -> VideoState {
        self.0.video.state()
    }

    #[must_use]
    pub fn soundtrack(&self) -> Soundtrack {
        match &self.0.lock().sound {
            Sound::Opening => Soundtrack::Opening,
            Sound::Open(_) => Soundtrack::Open,
            Sound::Silent => Soundtrack::Silent,
            Sound::Unplayable(error) => Soundtrack::Unplayable(Arc::clone(error)),
        }
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.0.lock().volume
    }

    #[must_use]
    pub fn is_looping(&self) -> bool {
        self.0.lock().looping
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Playback> {
        self.playback.lock()
    }

    fn tell(&self, event: MovieEvent) {
        let listener = self.listener.lock().clone();
        if let Some(listener) = listener {
            listener(event);
        }
    }

    fn heard(&self, event: VideoEvent) {
        let state = self.video.state();
        let mut told = match event {
            VideoEvent::Changed => MovieEvent::Changed,
            VideoEvent::Progressed => MovieEvent::Progressed,
        };
        let mut playback = self.lock();
        if playback.playing && matches!(state, VideoState::Ended | VideoState::Failed) {
            if let Some(voice) = playback.sound.voice() {
                voice.pause();
            }
            playback.playing = false;
            if state == VideoState::Ended {
                playback.ended = true;
                told = MovieEvent::Ended;
            }
        }
        self.follow(&mut playback, state);
        drop(playback);
        self.tell(told);
    }

    fn follow(&self, playback: &mut Playback, state: VideoState) {
        let picture = self.video.position();
        if let Some(target) = playback.target
            && has_arrived(target, picture, state)
        {
            playback.target = None;
        }
        let running = playback.playing && state == VideoState::Playing;
        let Some(voice) = playback.sound.voice().filter(|_| running) else {
            return;
        };
        if voice.is_paused() {
            voice.seek(picture);
            voice.play();
        } else if playback.target.is_none() && has_drifted(picture, voice.position()) {
            voice.seek(picture);
        }
    }

    fn move_to(self: &Arc<Self>, playback: &mut Playback, at: Duration) {
        playback.target = Some(at);
        playback.ended = false;
        self.video.seek(at);
        match &playback.sound {
            Sound::Open(voice) if voice.is_finished() => self.reopen(playback),
            Sound::Open(voice) => voice.seek(at),
            Sound::Opening | Sound::Silent | Sound::Unplayable(_) => {}
        }
    }

    fn resume(self: &Arc<Self>, playback: &mut Playback) {
        match &playback.sound {
            Sound::Open(voice) if voice.is_finished() => self.reopen(playback),
            Sound::Open(voice) => voice.play(),
            Sound::Opening | Sound::Silent | Sound::Unplayable(_) => {}
        }
    }

    fn reopen(self: &Arc<Self>, playback: &mut Playback) {
        playback.sound = Sound::Opening;
        let shared = Arc::downgrade(self);
        let path = self.path.clone();
        let started = thread::Builder::new()
            .name(OPENER.to_owned())
            .spawn(move || {
                let opened = Voice::open(&path);
                if let Some(shared) = shared.upgrade() {
                    shared.sounded(opened);
                }
            });
        if let Err(source) = started {
            playback.sound = Sound::Unplayable(Arc::new(Error::NoThread {
                name: OPENER,
                source,
            }));
        }
    }

    fn sounded(&self, opened: Result<Voice, Error>) {
        let mut playback = self.lock();
        playback.sound = match opened {
            Ok(voice) => {
                voice.set_volume(playback.volume);
                voice.set_looping(playback.looping);
                voice.seek(self.position_in(&playback));
                if playback.playing && self.video.state() == VideoState::Playing {
                    voice.play();
                }
                Sound::Open(voice)
            }
            Err(Error::Sound(rikka_audio::Error::Decode(
                rikka_audio_decode::Error::NoAudioTrack,
            ))) => Sound::Silent,
            Err(error) => Sound::Unplayable(Arc::new(error)),
        };
        drop(playback);
        self.tell(MovieEvent::Changed);
    }

    fn duration_in(&self, playback: &Playback) -> Option<Duration> {
        self.video
            .duration()
            .or_else(|| playback.sound.voice().and_then(Voice::duration))
    }

    fn position_in(&self, playback: &Playback) -> Duration {
        let duration = self.duration_in(playback);
        let position = match (playback.ended, duration) {
            (true, Some(duration)) => duration,
            _ => playback.target.unwrap_or_else(|| self.video.position()),
        };
        clamp_to(position, duration)
    }
}

fn clamp_to(position: Duration, duration: Option<Duration>) -> Duration {
    duration.map_or(position, |duration| position.min(duration))
}

impl std::fmt::Debug for Movie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Movie")
            .field("path", &self.0.path)
            .field("video", &self.0.video)
            .finish_non_exhaustive()
    }
}
