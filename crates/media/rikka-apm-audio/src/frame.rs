use crate::filter_bank::ThreeBandFilterBank;
use crate::{BAND, BANDS, FRAME, S16_SCALE};

#[derive(Clone, Debug)]
pub struct Frame {
    channels: usize,
    full: Vec<[f32; FRAME]>,
    bands: Vec<[[f32; BAND]; BANDS]>,
    banks: Vec<ThreeBandFilterBank>,
}

impl Frame {
    #[must_use]
    pub fn new(channels: usize) -> Frame {
        let channels = channels.max(1);
        Frame {
            channels,
            full: vec![[0.0; FRAME]; channels],
            bands: vec![[[0.0; BAND]; BANDS]; channels],
            banks: vec![ThreeBandFilterBank::new(); channels],
        }
    }

    #[must_use]
    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn set_channels(&mut self, channels: usize) {
        self.channels = channels.clamp(1, self.full.len());
    }

    #[must_use]
    pub fn full(&self, channel: usize) -> &[f32; FRAME] {
        &self.full[channel]
    }

    pub fn full_mut(&mut self, channel: usize) -> &mut [f32; FRAME] {
        &mut self.full[channel]
    }

    #[must_use]
    pub fn bands(&self, channel: usize) -> &[[f32; BAND]; BANDS] {
        &self.bands[channel]
    }

    pub fn bands_mut(&mut self, channel: usize) -> &mut [[f32; BAND]; BANDS] {
        &mut self.bands[channel]
    }

    #[must_use]
    pub fn band(&self, channel: usize, band: usize) -> &[f32; BAND] {
        &self.bands[channel][band]
    }

    pub fn band_mut(&mut self, channel: usize, band: usize) -> &mut [f32; BAND] {
        &mut self.bands[channel][band]
    }

    pub fn split(&mut self) {
        for channel in 0..self.channels {
            self.banks[channel].analysis(&self.full[channel], &mut self.bands[channel]);
        }
    }

    pub fn merge(&mut self) {
        for channel in 0..self.channels {
            self.banks[channel].synthesis(&self.bands[channel], &mut self.full[channel]);
        }
    }

    pub fn load(&mut self, interleaved: &[f32], channels: usize) {
        let channels = channels.max(1);
        self.channels = self.full.len();
        let own = self.channels;
        for (index, frame) in interleaved.chunks_exact(channels).take(FRAME).enumerate() {
            if own == channels {
                for (channel, sample) in frame.iter().enumerate() {
                    self.full[channel][index] = to_s16(*sample);
                }
            } else if own == 1 {
                let sum: f32 = frame.iter().sum();
                self.full[0][index] = to_s16(sum / channels as f32);
            } else {
                for channel in 0..own {
                    self.full[channel][index] = to_s16(frame[channel.min(channels - 1)]);
                }
            }
        }
    }

    pub fn store(&self, interleaved: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        for (index, frame) in interleaved
            .chunks_exact_mut(channels)
            .take(FRAME)
            .enumerate()
        {
            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = from_s16(self.full[channel.min(self.channels - 1)][index]);
            }
        }
    }
}

fn to_s16(sample: f32) -> f32 {
    sample.clamp(-1.0, 1.0) * S16_SCALE
}

fn from_s16(sample: f32) -> f32 {
    sample.clamp(-S16_SCALE, S16_SCALE) / S16_SCALE
}
