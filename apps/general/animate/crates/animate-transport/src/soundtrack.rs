use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rikka_audio::{PlayParams, Track};
use rikka_audio_decode::Sound;

const BUCKETS_PER_SECOND: f32 = 480.0;

pub struct Soundtrack {
    track: Track,
    peaks: Arc<Peaks>,
    name: String,
}

impl Soundtrack {
    pub fn read(path: &Path) -> rikka_audio::Result<Self> {
        let track = Track::load(path, PlayParams::new().start_paused())?;
        let peaks = track.sound().map(Peaks::measure).unwrap_or_default();
        Ok(Self {
            peaks: Arc::new(peaks),
            name: path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
            track,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn peaks(&self) -> &Arc<Peaks> {
        &self.peaks
    }

    pub fn sound(&self) -> Option<Sound> {
        self.track.sound().cloned()
    }

    pub fn length(&self) -> f32 {
        self.track
            .duration()
            .map_or(0.0, |duration| duration.as_secs_f32())
    }

    pub fn volume(&self) -> f32 {
        self.track.volume()
    }

    pub fn set_volume(&self, volume: f32) {
        self.track.set_volume(volume);
    }

    pub fn audible_time(&self) -> Option<f32> {
        self.track
            .audible_position()
            .map(|audible_time| audible_time.as_secs_f32())
    }

    pub fn play_from(&mut self, time: f32) {
        if time >= self.length() {
            self.track.pause();
            return;
        }
        self.track.play_from(Duration::from_secs_f32(time.max(0.0)));
    }

    pub fn pause(&self) {
        self.track.pause();
    }

    pub fn seek(&mut self, time: f32, playing: bool) {
        match playing {
            true => self.play_from(time),
            false => self.track.seek(Duration::from_secs_f32(time.max(0.0))),
        }
    }
}

impl std::fmt::Debug for Soundtrack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Soundtrack")
            .field("name", &self.name)
            .field("length", &self.length())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
pub struct Peaks {
    lows: Vec<f32>,
    highs: Vec<f32>,
    per_second: f32,
}

impl Peaks {
    fn measure(sound: &Sound) -> Self {
        Self::from_samples(
            sound.samples(),
            usize::from(sound.channels()),
            sound.sample_rate(),
        )
    }

    fn from_samples(samples: &[f32], channels: usize, rate: u32) -> Self {
        let frames = ((rate as f32 / BUCKETS_PER_SECOND).round() as usize).max(1);
        let (lows, highs) = samples
            .chunks(frames * channels.max(1))
            .map(|bucket| {
                bucket.iter().fold((0.0f32, 0.0f32), |(low, high), sample| {
                    (low.min(*sample), high.max(*sample))
                })
            })
            .unzip();
        Self {
            lows,
            highs,
            per_second: rate as f32 / frames as f32,
        }
    }

    pub fn within(&self, from: f32, to: f32) -> Option<(f32, f32)> {
        let bucket = |time: f32| (time.max(0.0) * self.per_second) as usize;
        let first = bucket(from);
        let last = bucket(to).max(first + 1).min(self.highs.len());
        if first >= last {
            return None;
        }
        let low = self.lows[first..last].iter().copied().fold(0.0, f32::min);
        let high = self.highs[first..last].iter().copied().fold(0.0, f32::max);
        Some((low.max(-1.0), high.min(1.0)))
    }
}
