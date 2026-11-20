use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};
use std::time::Duration;

use rikka_gpu::TextureFormat;
use rikka_gpu::{TextureId, Uploads, VideoDeviceHandles};

use crate::clock::PlaybackClock;
use crate::mailbox::{Mailbox, Wake};
use crate::worker::{Control, DecodedFrame, Event, Payload, Worker};
use rikka_video_core::VideoInfo;
use rikka_video_core::decode::Decoder;
use rikka_video_core::demux::Demuxer;
use rikka_video_core::hw::HwDevice;
use rikka_video_core::init;
use rikka_video_core::interop;
use rikka_video_core::reaper::{self, Reaped, Uninterruptible};
use rikka_video_core::{Error, Result};

const PRESENT_SLACK_US: i64 = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeBackend {
    Auto,
    VulkanVideo,
    Software,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Feed {
    Anchoring,
    Flowing,
    Draining,
}

pub struct VideoPlayer {
    texture: TextureId,
    info: VideoInfo,
    accelerated: bool,
    clock: PlaybackClock,
    state: PlaybackState,
    feed: Feed,
    generation: u64,
    video_device: Option<VideoDeviceHandles>,
    stashed: Option<DecodedFrame>,
    running: Running,
    fatal: Option<String>,
}

struct Running {
    mailbox: Arc<Mailbox>,
    _worker: Reaped<Arc<Mailbox>>,
}

#[derive(Debug)]
pub struct Opening {
    ready: Receiver<Result<Ready>>,
    _thread: Option<Reaped<Uninterruptible>>,
}

pub struct Ready {
    info: VideoInfo,
    accelerated: bool,
    video_device: Option<VideoDeviceHandles>,
    running: Running,
}

impl Opening {
    pub fn ready(&mut self) -> Option<Result<Ready>> {
        match self.ready.try_recv() {
            Ok(ready) => Some(ready),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(Error::Worker("video open thread exited".into())))
            }
        }
    }

    pub fn wait(self) -> Result<Ready> {
        self.ready
            .recv()
            .unwrap_or_else(|_| Err(Error::Worker("video open thread exited".into())))
    }
}

impl Ready {
    #[must_use]
    pub fn info(&self) -> &VideoInfo {
        &self.info
    }

    #[must_use]
    pub fn is_hardware_accelerated(&self) -> bool {
        self.accelerated
    }

    pub fn play_on(self, uploads: &Uploads) -> Result<VideoPlayer> {
        let texture = uploads.blank(self.info.width, self.info.height)?;
        Ok(VideoPlayer {
            texture,
            info: self.info,
            accelerated: self.accelerated,
            clock: PlaybackClock::new(),
            state: PlaybackState::Paused,
            feed: Feed::Anchoring,
            generation: 0,
            video_device: self.video_device,
            stashed: None,
            running: self.running,
            fatal: None,
        })
    }
}

impl std::fmt::Debug for Ready {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ready")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl VideoPlayer {
    pub fn open(uploads: &Uploads, path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with(uploads, path, DecodeBackend::Auto)
    }

    pub fn open_with(
        uploads: &Uploads,
        path: impl AsRef<Path>,
        backend: DecodeBackend,
    ) -> Result<Self> {
        let device = uploads.video_device();
        Self::opening(device, path.as_ref().to_path_buf(), backend, || {})
            .wait()?
            .play_on(uploads)
    }

    #[must_use]
    pub fn opening(
        device: Option<VideoDeviceHandles>,
        path: PathBuf,
        backend: DecodeBackend,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Opening {
        let wake: Wake = Arc::new(wake);
        let (sender, ready) = sync_channel(1);
        let answer = sender.clone();
        let started = reaper::spawn("rikka-video-open", Uninterruptible, move || {
            let prepared = prepare(device.as_ref(), &path, backend, Arc::clone(&wake));
            if sender.send(prepared).is_ok() {
                wake();
            }
        });
        let thread = match started {
            Ok(thread) => Some(thread),
            Err(error) => {
                if answer.send(Err(error)).is_err() {
                    tracing::debug!("the video was closed before it opened");
                }
                None
            }
        };
        Opening {
            ready,
            _thread: thread,
        }
    }

    pub fn texture(&self) -> TextureId {
        self.texture
    }

    pub fn info(&self) -> &VideoInfo {
        &self.info
    }

    #[must_use]
    pub fn is_hardware_accelerated(&self) -> bool {
        self.accelerated
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn position(&self) -> Duration {
        Duration::from_micros(self.clock.now_us().max(0) as u64)
    }

    pub fn play(&mut self) {
        if self.state == PlaybackState::Ended {
            self.seek(Duration::ZERO);
        }
        self.state = PlaybackState::Playing;
        if self.clock.is_running() || self.feed != Feed::Anchoring {
            self.clock.resume();
        }
    }

    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
        self.clock.pause();
    }

    pub fn seek(&mut self, position: Duration) {
        let target_us = position.as_micros().min(i64::MAX as u128) as i64;
        self.generation += 1;
        self.running.mailbox.send(Control::Seek {
            target_us,
            generation: self.generation,
        });
        self.stashed = None;
        if self.state == PlaybackState::Ended {
            self.state = PlaybackState::Paused;
        }
        self.feed = Feed::Anchoring;
        self.clock.set(target_us);
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.running.mailbox.send(Control::SetLooping(looping));
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.clock.set_speed(f64::from(speed));
    }

    #[must_use]
    pub fn next_due(&self) -> Option<Duration> {
        if self.state != PlaybackState::Playing {
            return None;
        }
        let stashed = self.stashed.as_ref()?;
        let ahead = self
            .clock
            .wall_us(stashed.pts_us - PRESENT_SLACK_US - self.clock.now_us());
        Some(Duration::from_micros(ahead.max(0) as u64))
    }

    pub fn advance(&mut self, uploads: &Uploads) -> Result<bool> {
        if let Some(message) = &self.fatal {
            return Err(Error::Worker(message.clone()));
        }
        let anchoring = self.feed == Feed::Anchoring;
        if self.state != PlaybackState::Playing && !anchoring {
            return Ok(false);
        }

        let candidate = self.next_due_frame();
        if let Some(message) = &self.fatal {
            return Err(Error::Worker(message.clone()));
        }
        let Some(frame) = candidate else {
            if self.feed == Feed::Draining && self.stashed.is_none() {
                self.state = PlaybackState::Ended;
                self.clock.pause();
            }
            return Ok(false);
        };

        if anchoring {
            match self.state {
                PlaybackState::Playing => self.clock.anchor(frame.pts_us),
                PlaybackState::Paused | PlaybackState::Ended => self.clock.set(frame.pts_us),
            }
            self.feed = Feed::Flowing;
        }
        self.present(uploads, frame)?;
        Ok(true)
    }

    fn next_due_frame(&mut self) -> Option<DecodedFrame> {
        let anchoring = self.feed == Feed::Anchoring;
        let deadline = (!anchoring).then(|| self.clock.now_us() + PRESENT_SLACK_US);
        let mut candidate = None;

        if let Some(frame) = self.stashed.take() {
            if frame.generation != self.generation {
            } else if deadline.is_none_or(|deadline| frame.pts_us <= deadline) {
                candidate = Some(frame);
            } else {
                self.stashed = Some(frame);
                return None;
            }
        }
        if anchoring && candidate.is_some() {
            return candidate;
        }

        while let Some(event) = self.running.mailbox.take() {
            match event {
                Event::Frame(frame) => {
                    if frame.generation != self.generation {
                        continue;
                    }
                    if let Some(deadline) = deadline
                        && frame.pts_us > deadline
                    {
                        self.stashed = Some(frame);
                        break;
                    }
                    if let Some(late) = candidate.replace(frame) {
                        tracing::trace!("dropping late frame at {}us", late.pts_us);
                    }
                    if anchoring {
                        break;
                    }
                }
                Event::EndOfStream { generation } => {
                    if generation == self.generation {
                        self.feed = Feed::Draining;
                    }
                }
                Event::Looped => {
                    self.feed = Feed::Anchoring;
                    break;
                }
                Event::Fatal(message) => {
                    self.fatal = Some(message);
                    break;
                }
            }
        }
        candidate
    }

    fn present(&mut self, uploads: &Uploads, frame: DecodedFrame) -> Result<()> {
        tracing::trace!(
            "presenting {}us (clock {}us)",
            frame.pts_us,
            self.clock.now_us()
        );
        match frame.payload {
            Payload::Hw(hw_frame) => {
                let device = self
                    .video_device
                    .as_ref()
                    .ok_or_else(|| Error::Unsupported("GPU frame without a video device".into()))?;
                let external = interop::to_external(hw_frame, device)?;
                unsafe { uploads.submit_video_frame(self.texture, external)? };
            }
            Payload::Sw {
                pixels,
                width,
                height,
            } => {
                if (width, height) != (self.info.width, self.info.height) {
                    tracing::warn!(
                        "skipping frame with changed resolution {width}x{height} \
                         (stream opened at {}x{})",
                        self.info.width,
                        self.info.height
                    );
                    return Ok(());
                }
                uploads.update(self.texture, TextureFormat::Rgba8Premultiplied, &pixels[..])?;
            }
        }
        Ok(())
    }

    pub fn close(self, uploads: &Uploads) {
        uploads.destroy(self.texture);
    }
}

impl std::fmt::Debug for VideoPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoPlayer")
            .field("texture", &self.texture)
            .field("info", &self.info)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

fn prepare(
    device: Option<&VideoDeviceHandles>,
    path: &Path,
    backend: DecodeBackend,
    wake: Wake,
) -> Result<Ready> {
    init::ensure()?;
    let demuxer = Demuxer::open(path)?;
    let info = VideoInfo::from_demuxer(&demuxer)?;

    let hw_device = hardware(device, &demuxer, backend)?;
    let decoder = Decoder::open(
        demuxer.decoder(),
        demuxer.stream().codecpar,
        demuxer.time_base(),
        hw_device.as_ref(),
    )?;
    let accelerated = decoder.is_hardware();
    let video_device = device.filter(|_| hw_device.is_some()).cloned();
    match accelerated {
        true => tracing::info!(
            "playing {}x{} via Vulkan Video decode",
            info.width,
            info.height
        ),
        false => tracing::info!("playing {}x{} via software decode", info.width, info.height),
    }

    let mailbox = Arc::new(Mailbox::new(wake));
    let worker = Worker::new(demuxer, decoder, Arc::clone(&mailbox))?;
    let running = reaper::spawn("rikka-video", Arc::clone(&mailbox), move || {
        let _hw_device = hw_device;
        worker.run();
    })?;

    Ok(Ready {
        info,
        accelerated,
        video_device,
        running: Running {
            mailbox,
            _worker: running,
        },
    })
}

fn hardware(
    device: Option<&VideoDeviceHandles>,
    demuxer: &Demuxer,
    backend: DecodeBackend,
) -> Result<Option<HwDevice>> {
    match backend {
        DecodeBackend::Software => Ok(None),
        DecodeBackend::Auto => match device {
            Some(handles) if demuxer.supports_vulkan() => match HwDevice::new(handles) {
                Ok(device) => Ok(Some(device)),
                Err(err) => {
                    tracing::warn!(
                        "failed to initialize Vulkan Video device, using software decode: {err}"
                    );
                    Ok(None)
                }
            },
            _ => Ok(None),
        },
        DecodeBackend::VulkanVideo => {
            let handles = device.ok_or_else(|| {
                Error::Unsupported("Vulkan Video decoding is not available on this device".into())
            })?;
            if !demuxer.supports_vulkan() {
                return Err(Error::Unsupported(
                    "this codec has no Vulkan Video decoder".into(),
                ));
            }
            Ok(Some(HwDevice::new(handles)?))
        }
    }
}
