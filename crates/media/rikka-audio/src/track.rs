use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::config::EngineConfig;
use crate::engine::AudioEngine;
use crate::error::Result;
use crate::handle::PlaybackHandle;
use crate::play_params::PlayParams;
use crate::watch::{PlaybackEvent, PlaybackWatch};
use rikka_audio_decode::Sound;

#[derive(Debug)]
enum Origin {
    Streamed(PathBuf),
    Loaded(Sound),
}

type Listener = dyn Fn(PlaybackEvent) + Send + Sync;

pub struct Track {
    playback: PlaybackHandle,
    origin: Origin,
    engine: AudioEngine,
    listener: Option<Arc<Listener>>,
    watch: Option<PlaybackWatch>,
}

impl Track {
    pub fn stream(path: impl AsRef<Path>, params: PlayParams) -> Result<Track> {
        let path = path.as_ref();
        let engine = AudioEngine::new(EngineConfig::new())?;
        let playback = engine.stream_with(path, params)?;
        Ok(Track {
            playback,
            origin: Origin::Streamed(path.to_path_buf()),
            engine,
            listener: None,
            watch: None,
        })
    }

    pub fn load(path: impl AsRef<Path>, params: PlayParams) -> Result<Track> {
        let engine = AudioEngine::new(EngineConfig::new())?;
        let sound = engine.load(path.as_ref())?;
        let playback = engine.play_with(&sound, params)?;
        Ok(Track {
            playback,
            origin: Origin::Loaded(sound),
            engine,
            listener: None,
            watch: None,
        })
    }

    #[must_use]
    pub fn sound(&self) -> Option<&Sound> {
        match &self.origin {
            Origin::Loaded(sound) => Some(sound),
            Origin::Streamed(_) => None,
        }
    }

    pub fn on_change(&mut self, listener: impl Fn(PlaybackEvent) + Send + Sync + 'static) {
        self.listener = Some(Arc::new(listener));
        self.rewatch();
    }

    pub fn play(&mut self) {
        match self.playback.is_finished() {
            true => self.restart(Duration::ZERO),
            false => self.playback.resume(),
        }
    }

    pub fn play_from(&mut self, at: Duration) {
        if self.playback.is_finished() {
            return self.restart(at);
        }
        self.playback.seek(at);
        self.playback.resume();
    }

    pub fn pause(&self) {
        self.playback.pause();
    }

    pub fn seek(&self, at: Duration) {
        if !self.playback.is_finished() {
            self.playback.seek(at);
        }
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        !self.playback.is_paused() && !self.playback.is_finished()
    }

    #[must_use]
    pub fn position(&self) -> Duration {
        self.playback.position()
    }

    #[must_use]
    pub fn audible_position(&self) -> Option<Duration> {
        let latency = self.engine.output_latency();
        self.is_playing()
            .then(|| self.playback.position().saturating_sub(latency))
    }

    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        match &self.origin {
            Origin::Loaded(sound) => Some(sound.duration()),
            Origin::Streamed(_) => self.playback.duration(),
        }
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.playback.volume()
    }

    pub fn set_volume(&self, volume: f32) {
        self.playback.set_volume(volume.clamp(0.0, 1.0));
    }

    fn restart(&mut self, at: Duration) {
        let params = PlayParams::new().start_at(at).volume(self.volume());
        let restarted = match &self.origin {
            Origin::Streamed(path) => self.engine.stream_with(path, params),
            Origin::Loaded(sound) => self.engine.play_with(sound, params),
        };
        match restarted {
            Ok(playback) => {
                self.playback = playback;
                self.rewatch();
            }
            Err(error) => tracing::warn!("failed to restart finished track: {error}"),
        }
    }

    fn rewatch(&mut self) {
        self.watch = self.listener.as_ref().map(|listener| {
            let listener = Arc::clone(listener);
            self.playback.watch(move |event| listener(event))
        });
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        self.playback.stop();
    }
}

impl std::fmt::Debug for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Track")
            .field("origin", &self.origin)
            .field("position", &self.position())
            .finish_non_exhaustive()
    }
}
