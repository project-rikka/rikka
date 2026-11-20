use std::time::Duration;

use crate::decoding::Pipeline;
use crate::error::Result;
use crate::read::AudioInfo;

pub struct Decoder {
    inner: Pipeline,
}

impl Decoder {
    pub(crate) fn new(inner: Pipeline) -> Decoder {
        Decoder { inner }
    }

    #[must_use]
    pub fn info(&self) -> &AudioInfo {
        self.inner.info()
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.inner.info().sample_rate
    }

    #[must_use]
    pub fn channels(&self) -> u16 {
        self.inner.info().channels
    }

    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.inner.info().duration
    }

    #[must_use]
    pub fn frames(&self) -> Option<u64> {
        self.inner.info().frames
    }

    pub fn seek(&mut self, to: Duration) -> Result<Duration> {
        self.inner.seek_to(to)
    }

    pub fn seek_frame(&mut self, frame: u64) -> Result<u64> {
        self.inner.seek(frame)
    }

    pub fn read(&mut self, samples: &mut Vec<f32>) -> Result<bool> {
        self.inner.next(samples)
    }
}

impl std::fmt::Debug for Decoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decoder")
            .field("info", self.inner.info())
            .finish_non_exhaustive()
    }
}
