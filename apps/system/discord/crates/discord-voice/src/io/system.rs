use std::os::fd::{AsFd, BorrowedFd};
use std::sync::Arc;
use std::time::Duration;

use discord_core::SAMPLE_RATE;
use rikka_audio::{
    AudioCapture, AudioEngine, AudioFeed, CaptureConfig, CaptureSource, DeviceWatch, EngineConfig,
    PlaybackHandle,
};
use rikka_resample::{ResampleQuality, Resampler};

use crate::error::Result;
use crate::io::{Devices, Endpoint, Hotplug, Microphone, Speaker};
use crate::output::OUTPUT_CHANNELS;

const CAPTURE_BUFFER: Duration = Duration::from_millis(500);
const READ_CHUNK: usize = 4096;
const PERIOD_FRAMES: u32 = SAMPLE_RATE / 100;

#[derive(Debug)]
pub(crate) struct System {
    hotplug: Arc<Hotplug>,
    _watch: DeviceWatch,
}

impl System {
    pub(crate) fn start() -> Result<System> {
        let hotplug = Arc::new(Hotplug::default());
        let notified = Arc::clone(&hotplug);
        let watch = DeviceWatch::start(move |_| notified.notify())?;
        Ok(System {
            hotplug,
            _watch: watch,
        })
    }
}

impl Devices for System {
    fn microphone(&self, name: Option<&str>) -> Result<Box<dyn Microphone>> {
        Ok(Box::new(SystemMicrophone::open(name)?))
    }

    fn speaker(&self, name: Option<&str>) -> Result<Box<dyn Speaker>> {
        Ok(Box::new(SystemSpeaker::open(name)?))
    }

    fn hotplug(&self) -> &Hotplug {
        &self.hotplug
    }
}

#[derive(Debug)]
struct SystemMicrophone {
    capture: AudioCapture,
    resampler: Resampler,
    channels: usize,
    raw: Vec<f32>,
    mono: Vec<f32>,
}

impl SystemMicrophone {
    fn open(name: Option<&str>) -> Result<SystemMicrophone> {
        let source = name.map_or(CaptureSource::Microphone, |name| {
            CaptureSource::Named(name.to_owned())
        });
        let config = CaptureConfig::default()
            .source(source)
            .buffered(CAPTURE_BUFFER)
            .period_frames(PERIOD_FRAMES);
        let capture = at_call_rate(
            || AudioCapture::start(config.clone().sample_rate(SAMPLE_RATE)),
            || AudioCapture::start(config.clone()),
        )?;
        let resampler = Resampler::with_quality(
            ResampleQuality::Balanced,
            capture.sample_rate(),
            SAMPLE_RATE,
            1,
        );
        Ok(SystemMicrophone {
            channels: usize::from(capture.channels().max(1)),
            capture,
            resampler,
            raw: Vec::with_capacity(READ_CHUNK * 2),
            mono: Vec::with_capacity(READ_CHUNK),
        })
    }
}

impl Endpoint for SystemMicrophone {
    fn is_lost(&self) -> bool {
        self.capture.is_device_lost()
    }
}

impl AsFd for SystemMicrophone {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.capture.as_fd()
    }
}

impl Microphone for SystemMicrophone {
    fn read(&mut self, into: &mut Vec<f32>) {
        loop {
            let start = self.raw.len();
            self.raw.resize(start + READ_CHUNK, 0.0);
            let read = self.capture.read(&mut self.raw[start..]);
            self.raw.truncate(start + read);
            let whole = self.raw.len() - self.raw.len() % self.channels;
            self.mono.clear();
            self.mono.extend(
                self.raw[..whole]
                    .chunks_exact(self.channels)
                    .map(|frame| frame.iter().sum::<f32>() / self.channels as f32),
            );
            self.raw.drain(..whole);
            self.resampler.process(&self.mono, into);
            if read < READ_CHUNK {
                return;
            }
        }
    }
}

#[derive(Debug)]
struct SystemSpeaker {
    feed: AudioFeed,
    handle: PlaybackHandle,
    engine: AudioEngine,
}

impl SystemSpeaker {
    fn open(name: Option<&str>) -> Result<SystemSpeaker> {
        let device = match name {
            Some(name) => EngineConfig::new().device_name(name),
            None => EngineConfig::new(),
        }
        .buffer_size(PERIOD_FRAMES);
        let engine = at_call_rate(
            || AudioEngine::new(device.clone().sample_rate(SAMPLE_RATE)),
            || AudioEngine::new(device.clone()),
        )?;
        let (handle, feed) = engine.feed(SAMPLE_RATE, OUTPUT_CHANNELS)?;
        Ok(SystemSpeaker {
            feed,
            handle,
            engine,
        })
    }
}

fn at_call_rate<T>(
    preferred: impl FnOnce() -> rikka_audio::Result<T>,
    native: impl FnOnce() -> rikka_audio::Result<T>,
) -> rikka_audio::Result<T> {
    preferred().or_else(|error| match error {
        rikka_audio::Error::NoChannels(_) | rikka_audio::Error::TooManyChannels { .. } => native(),
        other => Err(other),
    })
}

impl Endpoint for SystemSpeaker {
    fn is_lost(&self) -> bool {
        self.engine.is_device_lost()
    }
}

impl AsFd for SystemSpeaker {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.feed.as_fd()
    }
}

impl Speaker for SystemSpeaker {
    fn queued(&self) -> Duration {
        self.feed.queued()
    }

    fn watch_queued_below(&self, threshold: Duration) -> bool {
        self.feed.watch_queued_below(threshold)
    }

    fn underruns(&self) -> u32 {
        self.handle.underruns()
    }

    fn write(&mut self, stereo: &[f32]) -> Result<()> {
        Ok(self.feed.write(stereo)?)
    }
}
