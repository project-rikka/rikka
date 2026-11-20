use std::sync::Arc;
use std::time::Duration;

use rikka_resample::{ResampleQuality, resample};

#[derive(Debug, Clone)]
pub struct Sound {
    data: Arc<SoundData>,
}

#[derive(Debug)]
struct SoundData {
    samples: Vec<f32>,
    channels: u16,
    sample_rate: u32,
}

impl Sound {
    pub fn from_samples(samples: Vec<f32>, channels: u16, sample_rate: u32) -> Sound {
        assert!(channels > 0, "a sound needs at least one channel");
        assert_eq!(
            samples.len() % usize::from(channels),
            0,
            "sample count must be a multiple of the channel count",
        );
        Sound {
            data: Arc::new(SoundData {
                samples,
                channels,
                sample_rate,
            }),
        }
    }

    #[must_use]
    pub fn resampled(&self, sample_rate: u32, quality: ResampleQuality) -> Sound {
        if sample_rate == self.data.sample_rate {
            return self.clone();
        }
        let samples = resample(
            &self.data.samples,
            self.data.channels,
            self.data.sample_rate,
            sample_rate,
            quality,
        );
        Sound::from_samples(samples, self.data.channels, sample_rate)
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.data.sample_rate
    }

    #[must_use]
    pub fn channels(&self) -> u16 {
        self.data.channels
    }

    #[must_use]
    pub fn frames(&self) -> u64 {
        (self.data.samples.len() / usize::from(self.data.channels)) as u64
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        match self.data.sample_rate {
            0 => Duration::ZERO,
            rate => Duration::from_secs_f64(self.frames() as f64 / f64::from(rate)),
        }
    }

    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.data.samples
    }
}
