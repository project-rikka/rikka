#[cfg(unix)]
use std::os::fd::{AsFd, BorrowedFd};
#[cfg(windows)]
use std::os::windows::io::{AsHandle, BorrowedHandle};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::error::{Error, Result, Role};
use crate::frame::check_channels;
use crate::stream::{StreamShared, StreamSource};
use crate::voice::VoiceShared;
use rikka_resample::{ResampleQuality, Resampler};
use rikka_ring::Producer;

const QUEUED: Duration = Duration::from_millis(200);
const MIN_QUEUED_FRAMES: usize = 1024;
const STALL: Duration = Duration::from_secs(2);

pub struct AudioFeed {
    ring: Producer<f32>,
    shared: Arc<StreamShared>,
    voice: Arc<VoiceShared>,
    resampler: Resampler,
    outbox: Vec<f32>,
    channels: usize,
    output_rate: u32,
    flushes: u64,
}

pub(crate) struct FeedParts {
    pub(crate) feed: AudioFeed,
    pub(crate) source: StreamSource,
    pub(crate) shared: Arc<StreamShared>,
}

impl AudioFeed {
    pub(crate) fn open(
        sample_rate: u32,
        channels: u16,
        voice: Arc<VoiceShared>,
        output_rate: u32,
        resampler: ResampleQuality,
    ) -> Result<FeedParts> {
        check_channels(channels, Role::Feed)?;
        let capacity = ((f64::from(output_rate) * QUEUED.as_secs_f64()) as usize)
            .max(MIN_QUEUED_FRAMES)
            * usize::from(channels);
        let (producer, consumer) = rikka_ring::channel::<f32>(capacity);
        let shared = Arc::new(StreamShared::new(output_rate, None)?);
        let feed = AudioFeed {
            ring: producer,
            shared: Arc::clone(&shared),
            voice,
            resampler: Resampler::with_quality(resampler, sample_rate, output_rate, channels),
            outbox: Vec::new(),
            channels: usize::from(channels),
            output_rate,
            flushes: shared.flushes(),
        };
        let source = StreamSource::fed(consumer, Arc::clone(&shared), usize::from(channels));
        Ok(FeedParts {
            feed,
            source,
            shared,
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<()> {
        if !samples.len().is_multiple_of(self.channels) {
            return Err(Error::PartialFrame {
                samples: samples.len(),
                channels: self.channels,
            });
        }
        self.catch_up();
        let flushes = self.flushes;
        self.outbox.clear();
        self.resampler.process(samples, &mut self.outbox);

        let mut written = 0;
        let mut moved = Instant::now();
        while written < self.outbox.len() {
            if self.closed() {
                return Err(Error::FeedClosed);
            }
            if self.shared.flushes() != flushes {
                self.catch_up();
                return Ok(());
            }
            let paused = self.voice.paused.load(Ordering::Relaxed);
            if !self.shared.is_flushing() {
                let pushed = self.ring.push_slice(&self.outbox[written..]);
                if pushed > 0 || paused {
                    moved = Instant::now();
                }
                written += pushed;
            }
            if written == self.outbox.len() {
                break;
            }
            let Some(patience) = STALL.checked_sub(moved.elapsed()) else {
                return Err(Error::FeedStalled);
            };
            if paused || self.shared.is_flushing() {
                self.shared.wait_for_producer(Some(patience));
            } else {
                let pending = self.outbox.len() - written;
                let half_ring = (self.ring.free() + self.ring.len()) / 2;
                self.shared
                    .wait_for_room(&self.ring, pending.min(half_ring), Some(patience));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn queued(&self) -> Duration {
        let frames = self.ring.len() / self.channels;
        Duration::from_secs_f64(frames as f64 / f64::from(self.output_rate.max(1)))
    }

    pub fn watch_queued_below(&self, threshold: Duration) -> bool {
        let (threshold_samples, wanted_free) = self.room_below(threshold);
        self.shared.arm_room(&self.ring, wanted_free);
        self.ring.len() < threshold_samples || self.closed()
    }

    pub fn wait_until_queued_below(
        &self,
        threshold: Duration,
        timeout: impl Into<Option<Duration>>,
    ) -> bool {
        let (threshold_samples, wanted_free) = self.room_below(threshold);
        let deadline = timeout.into().map(|timeout| Instant::now() + timeout);
        loop {
            if self.ring.len() < threshold_samples {
                return true;
            }
            if self.closed() {
                return false;
            }
            let remaining = match deadline {
                Some(deadline) => match deadline.checked_duration_since(Instant::now()) {
                    Some(remaining) => Some(remaining),
                    None => return false,
                },
                None => None,
            };
            self.shared
                .wait_for_room(&self.ring, wanted_free, remaining);
        }
    }

    fn room_below(&self, threshold: Duration) -> (usize, usize) {
        let threshold_samples = (threshold.as_secs_f64() * f64::from(self.output_rate)).round()
            as usize
            * self.channels;
        let capacity = self.ring.free() + self.ring.len();
        (
            threshold_samples,
            (capacity + 1).saturating_sub(threshold_samples),
        )
    }

    fn catch_up(&mut self) {
        let flushes = self.shared.flushes();
        if flushes != self.flushes {
            self.flushes = flushes;
            self.resampler.reset();
        }
    }

    fn closed(&self) -> bool {
        self.voice.stop.load(Ordering::Relaxed) || self.voice.finished.load(Ordering::Relaxed)
    }
}

#[cfg(unix)]
impl AsFd for AudioFeed {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.shared.producer().as_fd()
    }
}

#[cfg(windows)]
impl AsHandle for AudioFeed {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.shared.producer().as_handle()
    }
}

impl Drop for AudioFeed {
    fn drop(&mut self) {
        self.shared.end();
    }
}

impl std::fmt::Debug for AudioFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioFeed")
            .field("channels", &self.channels)
            .field("output_rate", &self.output_rate)
            .finish_non_exhaustive()
    }
}
