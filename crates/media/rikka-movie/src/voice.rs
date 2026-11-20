use std::path::Path;
use std::time::Duration;

use rikka_audio::{AudioEngine, EngineConfig, PlayParams, PlaybackHandle};

use crate::error::Error;

pub(crate) struct Voice {
    handle: PlaybackHandle,
    _engine: AudioEngine,
}

impl Voice {
    pub(crate) fn open(path: &Path) -> Result<Voice, Error> {
        let engine = AudioEngine::new(EngineConfig::new())?;
        let handle = engine.stream_with(path, PlayParams::new().start_paused())?;
        Ok(Voice {
            handle,
            _engine: engine,
        })
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

    pub(crate) fn set_volume(&self, volume: f32) {
        self.handle.set_volume(volume);
    }

    pub(crate) fn set_looping(&self, looping: bool) {
        self.handle.set_looping(looping);
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.handle.is_paused()
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
