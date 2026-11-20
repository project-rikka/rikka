use rikka_sync::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rikka_movie::{Movie, MovieEvent, Soundtrack};
use rikka_ui::{VideoController, VideoState};

use crate::ffi;
use crate::media::check_file;
use crate::media::signals::Signals;
use crate::media::voice::Settings;
use crate::window::Listener;

#[derive(Debug)]
pub(crate) struct Video {
    path: PathBuf,
    missing: Option<String>,
    movie: Movie,
    signals: Signals,
    listener: Mutex<Option<Listener>>,
    playing: Mutex<Option<Arc<Video>>>,
}

impl Video {
    pub(crate) fn open(path: &str) -> Arc<Self> {
        let path = PathBuf::from(path);
        let video = Arc::new(Self {
            missing: check_file(&path).err(),
            movie: Movie::open(&path),
            path,
            signals: Signals::default(),
            listener: Mutex::new(None),
            playing: Mutex::new(None),
        });
        let heard = Arc::downgrade(&video);
        video.movie.on_change(move |event| {
            if let Some(video) = heard.upgrade() {
                video.heard(event);
            }
        });
        video
    }

    pub(crate) fn controller(&self) -> &VideoController {
        self.movie.video()
    }

    fn heard(&self, event: MovieEvent) {
        if !self.movie.is_playing() {
            self.let_go();
        }
        match event {
            MovieEvent::Changed => self.signals.changed(),
            MovieEvent::Progressed => self.signals.position_changed(),
            MovieEvent::Ended => {
                self.signals.changed();
                if let Some(listener) = self.listener.lock().clone() {
                    listener.notify();
                }
            }
        }
    }

    fn let_go(&self) {
        let released = self.playing.lock().take();
        drop(released);
    }

    pub(crate) fn play(self: &Arc<Self>) {
        if self.missing.is_some() {
            return;
        }
        *self.playing.lock() = Some(Arc::clone(self));
        self.movie.play();
    }

    pub(crate) fn pause(&self) {
        self.movie.pause();
        self.let_go();
    }

    pub(crate) fn stop(&self) {
        self.movie.pause();
        self.let_go();
        self.movie.seek(Duration::ZERO);
    }

    pub(crate) fn seek(&self, at: Duration) {
        self.movie.seek(at);
    }

    pub(crate) fn on_end(&self, listener: Listener) {
        *self.listener.lock() = Some(listener);
    }

    pub(crate) fn state(&self) -> ffi::PlaybackState {
        self.signals.read_state();
        if self.missing.is_some() {
            return ffi::PlaybackState::Failed;
        }
        let movie = &self.movie;
        match (movie.picture(), movie.is_ended(), movie.is_playing()) {
            (VideoState::Failed, _, _) => ffi::PlaybackState::Failed,
            (VideoState::Opening, _, _) => ffi::PlaybackState::Loading,
            (_, true, _) => ffi::PlaybackState::Ended,
            (_, false, true) => ffi::PlaybackState::Playing,
            (_, false, false) => ffi::PlaybackState::Paused,
        }
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.signals.read_state();
        self.movie.is_playing()
    }

    pub(crate) fn position(&self) -> Duration {
        self.signals.read_position();
        self.movie.position()
    }

    pub(crate) fn duration(&self) -> Option<Duration> {
        self.signals.read_state();
        self.movie.duration()
    }

    pub(crate) fn size(&self) -> Option<(f32, f32)> {
        self.signals.read_state();
        self.movie
            .video()
            .size()
            .map(|size| (size.width, size.height))
    }

    pub(crate) fn settings(&self) -> Settings {
        self.signals.read_state();
        self.current_settings()
    }

    pub(crate) fn set_settings(&self, change: impl FnOnce(&mut Settings)) {
        let mut settings = self.current_settings();
        change(&mut settings);
        self.movie.set_volume(settings.volume);
        self.movie.set_looping(settings.looping);
    }

    fn current_settings(&self) -> Settings {
        Settings {
            volume: self.movie.volume(),
            looping: self.movie.is_looping(),
        }
    }

    pub(crate) fn error_message(&self) -> String {
        self.signals.read_state();
        if let Some(missing) = &self.missing {
            return missing.clone();
        }
        if let Some(error) = self.movie.video().error() {
            return format!("failed to play {}: {error}", self.path.display());
        }
        match self.movie.soundtrack() {
            Soundtrack::Unplayable(error) => {
                format!(
                    "failed to play the sound of {}: {error}",
                    self.path.display()
                )
            }
            Soundtrack::Opening | Soundtrack::Open | Soundtrack::Silent => String::new(),
        }
    }
}
