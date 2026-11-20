pub(crate) mod intake;

use rikka_sync::{Mutex, MutexGuard};
#[cfg(unix)]
use std::os::fd::{AsFd, BorrowedFd};
#[cfg(windows)]
use std::os::windows::io::{AsHandle, BorrowedHandle};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::backend::{self, CaptureHost};
use crate::error::Result;
use rikka_ring::Consumer;
use rikka_worker::{EventFd, event_fd};

const LEAST_BUFFERED: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AudioApp {
    pub key: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CaptureKind {
    #[default]
    Microphone,
    SystemAudio,
}

#[derive(Clone, Debug)]
pub struct CaptureDevice {
    pub name: String,
    pub kind: CaptureKind,
    pub is_default: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CaptureSource {
    #[default]
    Microphone,
    SystemAudio,
    OtherApps,
    Apps(Vec<String>),
    Named(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Filled,
    Silent,
    Ended,
}

#[derive(Clone, Debug)]
pub struct CaptureConfig {
    pub(crate) source: CaptureSource,
    pub(crate) sample_rate: Option<u32>,
    pub(crate) channels: Option<u16>,
    pub(crate) period: Option<u32>,
    pub(crate) node: Option<String>,
    pub(crate) passive: bool,
    buffered: Duration,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            source: CaptureSource::default(),
            sample_rate: None,
            channels: None,
            period: None,
            node: None,
            passive: false,
            buffered: Duration::from_secs(2),
        }
    }
}

impl CaptureConfig {
    pub fn source(mut self, source: CaptureSource) -> Self {
        self.source = source;
        self
    }

    pub fn sample_rate(mut self, hz: u32) -> Self {
        self.sample_rate = Some(hz);
        self
    }

    pub fn channels(mut self, channels: u16) -> Self {
        self.channels = Some(channels);
        self
    }

    pub fn period_frames(mut self, frames: u32) -> Self {
        self.period = Some(frames.max(1));
        self
    }

    pub fn buffered(mut self, span: Duration) -> Self {
        self.buffered = span;
        self
    }

    pub fn node_name(mut self, name: impl Into<String>) -> Self {
        self.node = Some(name.into());
        self
    }

    pub fn passive(mut self) -> Self {
        self.passive = true;
        self
    }

    pub(crate) fn capacity(&self, rate: u32, channels: u16) -> usize {
        let frames = (self.buffered.as_secs_f64() * f64::from(rate)) as usize;
        (frames * usize::from(channels)).max(LEAST_BUFFERED)
    }
}

pub(crate) struct Opened {
    pub(crate) consumer: Consumer<f32>,
    pub(crate) rate: u32,
    pub(crate) channels: u16,
}

pub struct AudioCapture {
    host: CaptureHost,
    consumer: Mutex<Consumer<f32>>,
    readable: EventFd,
    sample_rate: u32,
    channels: u16,
}

impl AudioCapture {
    pub fn devices() -> Result<Vec<CaptureDevice>> {
        backend::capture_devices()
    }

    pub fn apps() -> Result<Vec<AudioApp>> {
        backend::apps()
    }

    pub fn start(config: CaptureConfig) -> Result<Self> {
        let (arrived, readable) = event_fd()?;
        let (host, opened) = CaptureHost::start(config, arrived)?;
        Ok(Self {
            host,
            consumer: Mutex::new(opened.consumer),
            readable,
            sample_rate: opened.rate,
            channels: opened.channels,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn available(&self) -> usize {
        self.reader().len()
    }

    pub fn read(&self, out: &mut [f32]) -> usize {
        self.readable.drain();
        let channels = usize::from(self.channels);
        let whole = out.len() - out.len() % channels;
        self.reader().pop_slice(&mut out[..whole])
    }

    pub fn fill(&self, frames: &mut [f32], wait: Duration) -> Fill {
        let whole = frames.len() - frames.len() % usize::from(self.channels);
        if self.wait_until(wait, |capture| capture.available() >= whole) {
            self.reader().pop_slice(&mut frames[..whole]);
            return Fill::Filled;
        }
        match self.is_device_lost() {
            true => Fill::Ended,
            false => Fill::Silent,
        }
    }

    pub fn wait_readable(&self, timeout: Duration) -> bool {
        self.wait_until(timeout, |capture| capture.available() > 0)
    }

    fn wait_until(&self, timeout: Duration, ready: impl Fn(&Self) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            self.readable.drain();
            if ready(self) {
                return true;
            }
            let now = Instant::now();
            if self.is_device_lost() || now >= deadline {
                return false;
            }
            if let Err(error) = self.readable.wait_timeout(deadline - now) {
                tracing::warn!("failed to wait for captured sound: {error}");
                return false;
            }
        }
    }

    fn reader(&self) -> MutexGuard<'_, Consumer<f32>> {
        self.consumer.lock()
    }

    pub fn dropped(&self) -> u64 {
        self.host.dropped.load(Ordering::Relaxed)
    }

    pub fn is_device_lost(&self) -> bool {
        self.host.device_lost.load(Ordering::Relaxed)
    }
}

#[cfg(unix)]
impl AsFd for AudioCapture {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.readable.as_fd()
    }
}

#[cfg(windows)]
impl AsHandle for AudioCapture {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.readable.as_handle()
    }
}

impl std::fmt::Debug for AudioCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioCapture")
            .field("sample_rate", &self.sample_rate)
            .field("channels", &self.channels)
            .field("dropped", &self.dropped())
            .finish_non_exhaustive()
    }
}
