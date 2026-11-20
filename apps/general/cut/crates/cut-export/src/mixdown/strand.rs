use std::path::Path;
use std::time::Duration;

use cut_project::Alignment;
use rikka_audio_decode::Decoder;
use rikka_resample::{ResampleQuality, Resampler};

const COMPACT: usize = 1 << 16;

pub(super) struct Strand {
    pub(super) layer: usize,
    pub(super) alignment: Alignment,
    pub(super) used: bool,
    feed: Option<Feed>,
}

struct Feed {
    decoder: Decoder,
    resampler: Resampler,
    decoded: Vec<f32>,
    ready: Vec<f32>,
    taken: usize,
    ended: bool,
    failed: bool,
}

impl Strand {
    pub(super) fn open(
        layer: usize,
        alignment: Alignment,
        path: &Path,
        from: Duration,
        sample_rate: u32,
    ) -> Result<Strand, rikka_audio_decode::Error> {
        let mut decoder = rikka_audio_decode::read(path).decoder()?;
        let skip = match decoder.seek(from) {
            Ok(reached) => from.saturating_sub(reached),
            Err(error) => {
                tracing::debug!(%error, path = %path.display(), "seek failed, decoding from start");
                decoder = rikka_audio_decode::read(path).decoder()?;
                from
            }
        };
        let channels = decoder.channels().max(1);
        let mut feed = Feed {
            resampler: Resampler::at_speed(
                ResampleQuality::High,
                decoder.sample_rate(),
                sample_rate,
                alignment.speed,
                channels,
            ),
            decoder,
            decoded: Vec::new(),
            ready: Vec::new(),
            taken: 0,
            ended: false,
            failed: false,
        };
        feed.skip(skip)?;
        Ok(Strand {
            layer,
            alignment,
            used: true,
            feed: Some(feed),
        })
    }

    pub(super) fn silent(layer: usize, alignment: Alignment) -> Strand {
        Strand {
            layer,
            alignment,
            used: true,
            feed: None,
        }
    }

    pub(super) fn channels(&self) -> u16 {
        self.feed
            .as_ref()
            .map_or(1, |feed| feed.decoder.channels().max(1))
    }

    pub(super) fn failed(&self) -> bool {
        self.feed.as_ref().is_some_and(|feed| feed.failed)
    }

    pub(super) fn pull(&mut self, frames: usize) -> Option<&[f32]> {
        self.feed.as_mut().map(|feed| feed.pull(frames))
    }
}

impl Feed {
    fn skip(&mut self, by: Duration) -> Result<(), rikka_audio_decode::Error> {
        let channels = usize::from(self.decoder.channels().max(1));
        let mut left =
            (by.as_secs_f64() * f64::from(self.decoder.sample_rate())).round() as usize * channels;
        while left > 0 {
            self.decoded.clear();
            if !self.decoder.read(&mut self.decoded)? {
                self.ended = true;
                return Ok(());
            }
            let dropped = left.min(self.decoded.len());
            left -= dropped;
            if dropped < self.decoded.len() {
                self.resampler
                    .process(&self.decoded[dropped..], &mut self.ready);
            }
        }
        Ok(())
    }

    fn pull(&mut self, frames: usize) -> &[f32] {
        let channels = usize::from(self.decoder.channels().max(1));
        let samples = frames * channels;
        if self.taken >= COMPACT {
            self.ready.drain(..self.taken);
            self.taken = 0;
        }
        while self.ready.len() - self.taken < samples {
            if self.ended {
                self.ready.resize(self.taken + samples, 0.0);
                break;
            }
            self.decoded.clear();
            match self.decoder.read(&mut self.decoded) {
                Ok(true) => self.resampler.process(&self.decoded, &mut self.ready),
                Ok(false) => {
                    self.resampler.flush(&mut self.ready);
                    self.ended = true;
                }
                Err(error) => {
                    tracing::warn!(%error, "sound decoding stopped");
                    self.resampler.flush(&mut self.ready);
                    self.ended = true;
                    self.failed = true;
                }
            }
        }
        let from = self.taken;
        self.taken += samples;
        &self.ready[from..from + samples]
    }
}
