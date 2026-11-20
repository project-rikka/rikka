use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cut_project::Alignment;
use cut_project::Level;
use rikka_audio::{AudioEngine, EngineConfig, PlayParams, PlaybackHandle};

const DRIFT: Duration = Duration::from_millis(80);
const STEADY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SourceLine {
    pub(crate) layer: usize,
    pub(crate) alignment: Alignment,
}

impl SourceLine {
    pub(crate) fn continues(&self, other: &SourceLine) -> bool {
        self.layer == other.layer && self.alignment.continues(&other.alignment)
    }
}

#[derive(Debug)]
pub(crate) struct Cue<'a> {
    pub(crate) strand: SourceLine,
    pub(crate) path: &'a Path,
    pub(crate) source: Duration,
    pub(crate) level: Level,
    pub(crate) gain: f32,
}

impl Cue<'_> {
    fn volume(&self) -> f32 {
        self.level.gain * self.gain
    }
}

#[derive(Debug)]
struct Voice {
    strand: SourceLine,
    path: PathBuf,
    handle: PlaybackHandle,
    steadied: Instant,
}

impl Voice {
    fn follows(&self, cue: &Cue<'_>) -> bool {
        cue.strand.continues(&self.strand) && cue.path == self.path
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        self.handle.stop();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Undecodable,
    Vanished,
}

enum Engine {
    Unopened,
    Open(AudioEngine),
    Missing,
}

pub(crate) struct Voices {
    engine: Engine,
    volume: f32,
    voices: Vec<Voice>,
    refused: HashMap<PathBuf, Refusal>,
}

impl Default for Voices {
    fn default() -> Voices {
        Voices {
            engine: Engine::Unopened,
            volume: 1.0,
            voices: Vec::new(),
            refused: HashMap::new(),
        }
    }
}

impl std::fmt::Debug for Voices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Voices")
            .field("voices", &self.voices)
            .field("refused", &self.refused)
            .finish_non_exhaustive()
    }
}

impl Voices {
    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        if let Engine::Open(engine) = &self.engine {
            engine.set_master_volume(volume);
        }
    }

    pub(crate) fn silence(&mut self) {
        self.voices.clear();
    }

    pub(crate) fn follow(&mut self, sounding: &[Cue<'_>], coming: &[Cue<'_>], now: Instant) {
        self.voices
            .retain(|voice| sounding.iter().chain(coming).any(|cue| voice.follows(cue)));
        for cue in sounding {
            match self.voices.iter_mut().find(|voice| voice.follows(cue)) {
                Some(voice) => {
                    voice.strand = cue.strand;
                    mixed(&voice.handle, cue);
                    match voice.handle.is_paused() {
                        true => {
                            voice.handle.resume();
                            voice.steadied = now;
                        }
                        false => keep_in_step(voice, cue.source, now),
                    }
                }
                None => self.start(cue, false, now),
            }
        }
        for cue in coming {
            if !self.voices.iter().any(|voice| voice.follows(cue)) {
                self.start(cue, true, now);
            }
        }
    }

    pub(crate) fn forget_vanished(&mut self) {
        self.refused
            .retain(|_, refusal| *refusal != Refusal::Vanished);
    }

    fn start(&mut self, cue: &Cue<'_>, paused: bool, now: Instant) {
        if self.refused.contains_key(cue.path) {
            return;
        }
        let Some(engine) = self.engine() else {
            return;
        };
        let params = PlayParams::new()
            .start_at(cue.source)
            .speed(cue.strand.alignment.speed)
            .volume(cue.volume())
            .pan(cue.level.pan);
        let params = match paused {
            true => params.start_paused(),
            false => params,
        };
        match engine.stream_with(cue.path, params) {
            Ok(handle) => self.voices.push(Voice {
                strand: cue.strand,
                path: cue.path.to_path_buf(),
                handle,
                steadied: now,
            }),
            Err(error) => {
                tracing::warn!(%error, path = %cue.path.display(), "cannot decode sound");
                let refusal = match cue.path.is_file() {
                    true => Refusal::Undecodable,
                    false => Refusal::Vanished,
                };
                self.refused.insert(cue.path.to_path_buf(), refusal);
            }
        }
    }

    fn engine(&mut self) -> Option<&AudioEngine> {
        if let Engine::Unopened = self.engine {
            self.engine = match AudioEngine::new(EngineConfig::new()) {
                Ok(engine) => {
                    engine.set_master_volume(self.volume);
                    Engine::Open(engine)
                }
                Err(error) => {
                    tracing::warn!(%error, "cannot open audio output");
                    Engine::Missing
                }
            };
        }
        match &self.engine {
            Engine::Open(engine) => Some(engine),
            Engine::Unopened | Engine::Missing => None,
        }
    }
}

fn mixed(handle: &PlaybackHandle, cue: &Cue<'_>) {
    handle.set_volume(cue.volume());
    handle.set_pan(cue.level.pan);
}

fn keep_in_step(voice: &mut Voice, source: Duration, now: Instant) {
    if now.saturating_duration_since(voice.steadied) < STEADY || voice.handle.is_finished() {
        return;
    }
    if voice.handle.position().abs_diff(source) > DRIFT {
        voice.handle.seek(source);
        voice.steadied = now;
    }
}
