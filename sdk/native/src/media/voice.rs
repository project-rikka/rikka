use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rikka_audio::{AudioEngine, PlayParams, PlaybackEvent, PlaybackHandle, PlaybackWatch};

use crate::media::engine;

const OPENER: &str = "rikka-sdk-open";

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    pub(crate) volume: f32,
    pub(crate) looping: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            looping: false,
        }
    }
}

pub(crate) struct Voice {
    handle: PlaybackHandle,
    watch: Option<PlaybackWatch>,
    _engine: Arc<AudioEngine>,
}

impl Voice {
    fn open(path: &Path, settings: Settings) -> rikka_audio::Result<Self> {
        let engine = engine::shared()?;
        let params = PlayParams::new()
            .start_paused()
            .volume(settings.volume)
            .looping(settings.looping);
        let handle = engine.stream_with(path, params)?;
        Ok(Self {
            handle,
            watch: None,
            _engine: engine,
        })
    }

    pub(crate) fn open_for<T: Send + Sync + 'static>(
        owner: &Arc<T>,
        path: PathBuf,
        settings: Settings,
        opened: fn(&Arc<T>, rikka_audio::Result<Voice>),
    ) -> Result<(), String> {
        let owner = Arc::downgrade(owner);
        thread::Builder::new()
            .name(OPENER.to_owned())
            .spawn(move || {
                let voice = Self::open(&path, settings);
                if let Some(owner) = owner.upgrade() {
                    opened(&owner, voice);
                }
            })
            .map(drop)
            .map_err(|error| format!("failed to start {OPENER}: {error}"))
    }

    pub(crate) fn watch(&mut self, listener: impl Fn(PlaybackEvent) + Send + Sync + 'static) {
        self.watch = Some(self.handle.watch(listener));
    }

    pub(crate) fn apply(&self, settings: Settings) {
        self.handle.set_volume(settings.volume);
        self.handle.set_looping(settings.looping);
    }

    pub(crate) fn play(&self) {
        self.handle.resume();
    }

    pub(crate) fn pause(&self) {
        self.handle.pause();
    }

    pub(crate) fn seek(&self, at: Duration) {
        self.handle.seek(at);
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    pub(crate) fn position(&self) -> Duration {
        self.handle.position()
    }

    pub(crate) fn duration(&self) -> Option<Duration> {
        self.handle.duration()
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        self.handle.stop();
    }
}

impl std::fmt::Debug for Voice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Voice")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}
